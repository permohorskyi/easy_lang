use std::future;
use std::future::{Future, ready, Ready};
use std::pin::Pin;
use std::rc::Rc;

use actix_web::{dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform}, Either, Error, HttpMessage, HttpResponse, ResponseError, web};
use actix_web::body::{EitherBody, MessageBody};
use actix_web::cookie::Cookie;
use actix_web::http::header;
use chrono::Utc;
use futures_util::future::LocalBoxFuture;
use futures_util::FutureExt;
use jsonwebtoken::{decode, DecodingKey, Validation};
use sqlx::error::DatabaseError;
use crate::cookie::create_cookie_auth_clear;
use crate::jwt::{Claims, validate_token};
use crate::models::{MyError, MysqlInfo};
use crate::StateDb;

// There are two steps in middleware processing.
// 1. Middleware initialization, middleware factory gets called with
//    next service in chain as parameter.
// 2. Middleware's call method gets called with normal request.
pub struct CheckAuth;

// Middleware factory is `Transform` trait
// `S` - type of the next service
// `B` - type of response's body
impl<S, B> Transform<S, ServiceRequest> for CheckAuth
    where
        S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>+ 'static,
        S::Future: 'static,
        B: 'static,
{

    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = CheckAuthMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(CheckAuthMiddleware { service: Rc::new(service) }))
    }
}

pub struct CheckAuthMiddleware<S> {
    service: Rc<S>,
}
fn extract_cookie(req: &ServiceRequest, cookie_name: &str) -> Option<String> {
    req.headers()
        .get(header::COOKIE)
        .and_then(|header_value| header_value.to_str().ok())
        .and_then(|cookies| {
            cookies
                .split(';')
                .find_map(|cookie| {
                    let mut parts = cookie.trim().splitn(2, '=');
                    if parts.next()? == cookie_name {
                        parts.next().map(|value| value.to_string())
                    } else {
                        None
                    }
                })
        })
}

impl<S, B> Service<ServiceRequest> for CheckAuthMiddleware<S>
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
            let token=extract_cookie(&req,"refresh_token");
            let cookie =create_cookie_auth_clear();
            let response = HttpResponse::Found()
                .insert_header((http::header::LOCATION, "/view/login")).cookie(cookie)
                .finish().map_into_right_body();

            match token {
                None => {
                    Ok(ServiceResponse::new(req.into_parts().0, response))
                }
                Some(some) => {
                    match validate_token(some) {
                        Ok(claim) => {
                            req.extensions_mut().insert(claim);
                            service.call(req).await.map(ServiceResponse::map_into_left_body)
                        }
                        Err(e) => {
                            Ok(ServiceResponse::new(req.into_parts().0, response))
                        }
                    }

                }
            }
            // if azs_db.mysql.is_none() {
            //     drop(azs_db);
            //     let response = HttpResponse::Found()
            //         .insert_header((http::header::LOCATION, "/settings/dbproperties"))
            //         .finish().map_into_right_body();
            //     Ok(ServiceResponse::new(req.into_parts().0, response))
            // } else {
            //     drop(azs_db);
            //     service.call(req).await.map(ServiceResponse::map_into_left_body)
            // }
        })

    }
}