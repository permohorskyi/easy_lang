use std::future;
use std::future::{Future, ready, Ready};
use std::pin::Pin;
use std::rc::Rc;

use actix_web::{dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform}, Either, Error, HttpResponse, ResponseError, web};
use actix_web::body::{EitherBody, MessageBody};
use actix_web::http::header;
use futures_util::future::LocalBoxFuture;
use futures_util::FutureExt;
use serde::{Deserialize, Serialize};
use sqlx::error::DatabaseError;
use sqlx::FromRow;
use crate::controllers::object_of_controller::ErrorDb;
use crate::models::MyError;
use crate::StateDb;

// There are two steps in middleware processing.
// 1. Middleware initialization, middleware factory gets called with
//    next service in chain as parameter.
// 2. Middleware's call method gets called with normal request.
pub struct CheckDbApi;

// Middleware factory is `Transform` trait
// `S` - type of the next service
// `B` - type of response's body
impl<S, B> Transform<S, ServiceRequest> for CheckDbApi
    where
        S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>+ 'static,
        S::Future: 'static,
        B: 'static,
{

    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = CheckDbApiMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(CheckDbApiMiddleware { service: Rc::new(service) }))
    }
}

pub struct CheckDbApiMiddleware<S> {
    service: Rc<S>,
}


impl<S, B> Service<ServiceRequest> for CheckDbApiMiddleware<S>
    where
        S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>+ 'static,
        S::Future: 'static,
        B: 'static,
{

    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future =LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let service = self.service.clone();
        Box::pin(async move {
            let state = req.app_data::<web::Data<StateDb>>().unwrap();
            println!("Hi from start. You requested: {}", req.path());
            let mysql_db = state.mysql_db.lock().await;

            if mysql_db.mysql.is_none() {
                drop(mysql_db);

                let response = HttpResponse::Ok().json(ErrorDb{error:true}).map_into_right_body();
                Ok(ServiceResponse::new(req.into_parts().0, response))
            } else {
                drop(mysql_db);
                // service.call(req).await.map(ServiceResponse::map_into_left_body)

                let mysql_pointer=state.mysql_db.clone();
                let url=req.uri().to_string();
                match service.call(req).await {
                    Ok(service_response) => {

                        if let Some(err) = service_response.response().error() {
                            if let Some(my_error) = err.as_error::<MyError>() {
                                let mut mysql_db=mysql_pointer.lock().await;
                                mysql_db.disconnect().await;
                                let updated_error = MyError::SiteError(format!("{} URL ERROR: {} ", my_error, url));
                                updated_error.pushlog().await;
                                // Використання оновленої помилки для створення нової HTTP відповіді
                                let response = HttpResponse::Ok().json(ErrorDb{error:true});
                                let new_serv=ServiceResponse::new(service_response.request().clone(), response);
                                // Потенційно змініть відповідь залежно від типу помилки
                                return Ok(new_serv.map_into_right_body());
                            }
                        }
                        Ok(service_response.map_into_left_body())
                    },
                    Err(e) => {
                        Err(e)
                    },
                }



            }
        })

    }
}