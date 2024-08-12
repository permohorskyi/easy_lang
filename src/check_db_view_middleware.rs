use std::future;
use std::future::{Future, ready, Ready};
use std::pin::Pin;
use std::rc::Rc;

use actix_web::{dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform}, Either, Error, HttpResponse, ResponseError, web};
use actix_web::body::{EitherBody, MessageBody};
use actix_web::http::header;
use futures_util::future::LocalBoxFuture;
use futures_util::FutureExt;
use sqlx::error::DatabaseError;
use crate::models::MyError;
use crate::StateDb;

// There are two steps in middleware processing.
// 1. Middleware initialization, middleware factory gets called with
//    next service in chain as parameter.
// 2. Middleware's call method gets called with normal request.
pub struct CheckDbView;

// Middleware factory is `Transform` trait
// `S` - type of the next service
// `B` - type of response's body
impl<S, B> Transform<S, ServiceRequest> for CheckDbView
    where
        S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>+ 'static,
        S::Future: 'static,
        B: 'static,
{

    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = CheckDbViewMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(CheckDbViewMiddleware { service: Rc::new(service) }))
    }
}

pub struct CheckDbViewMiddleware<S> {
    service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for CheckDbViewMiddleware<S>
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
                let response = HttpResponse::Found()
                    .insert_header((http::header::LOCATION, "/settings/error"))
                    .finish().map_into_right_body();
                Ok(ServiceResponse::new(req.into_parts().0, response))
            } else {
                drop(mysql_db);
               // service.call(req).await.map(ServiceResponse::map_into_left_body)

                let azs_pointer=state.mysql_db.clone();
                let url=req.uri().to_string();
                match service.call(req).await {
                    Ok(service_response) => {

                        if let Some(err) = service_response.response().error() {
                            if let Some(my_error) = err.as_error::<MyError>() {
                                let mut azs_db=azs_pointer.lock().await;
                                azs_db.disconnect().await;
                                let updated_error = MyError::SiteError(format!("{} URL ERROR: {} ", my_error, url));

                                // Використання оновленої помилки для створення нової HTTP відповіді
                                let response = updated_error.error_response();
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

                // match res {
                //     Ok(service_response) => Ok(service_response),
                //     Err(e) => {
                //         if let Some(custom_error) = e.as_error::<MyError>() {
                //             // Handle your custom error
                //             // You can modify the response here based on your error
                //         }
                //         Err(e)
                //     },
                // }

            }
        })
        // let state = req.app_data::<web::Data<StateDb>>().unwrap();
        // let azs_db=state.azs_db.lock().await;
        // if azs_db.mysql.is_none(){
        //
        //     // Якщо умова виконується, робіть редірект
        //     let response = HttpResponse::Found()
        //         .insert_header((http::header::LOCATION, "/settings/dbproperties"))
        //         .finish().map_into_right_body();
        //     Box::pin(async move {
        //         Ok(ServiceResponse::new(req.into_parts().0, response))
        //     })
        // } else {
        //     let fut = self.service.call(req);
        //     // Якщо умова не виконується, продовжуйте до наступного сервісу
        //     Box::pin(async move {
        //         fut.await.map(ServiceResponse::map_into_left_body)
        //     })
        // }

            // Перевірте свою умову тут
            // if azs_db.mysql.is_none() {
            //
            //     // Якщо умова виконується, робіть редірект
            //     let response = HttpResponse::Found()
            //         .insert_header((http::header::LOCATION, "/settings/dbproperties"))
            //         .finish().map_into_right_body();
            //     Box::pin(async move {
            //         Ok(ServiceResponse::new(req.into_parts().0, response))
            //     })
            // } else {
            //
            //     // Якщо умова не виконується, продовжуйте до наступного сервісу
            //     Box::pin(async move {
            //
            //     })
            // }
    }
}