#![allow(dead_code)]
use actix_web::{http::StatusCode, HttpResponse, ResponseError};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse {
    pub code: u16,
    pub message: String,
}

impl ErrorResponse {
    pub fn new(code: StatusCode, message: String) -> Self {
        Self {
            code: code.as_u16(),
            message,
        }
    }
}

#[derive(Error, Debug)]
pub enum XProtocolError {
    #[error("insufficient storage")]
    InsufficientStorage,
    #[error("not found")]
    NotFound,
    #[error("bad requests")]
    BadRequest,
    #[error("Method Not Allowed")]
    MethodNotAllowed,
    #[error("Expectation Failed")]
    ExpectationFailed,
    #[error("Internal Server Error")]
    InternalServerError,
    #[error("Chain Id Not Found")]
    ChainIdNotFound,
    #[error("unknown")]
    Unknown,
    #[error("overflow")]
    Overflow,
    #[error("Insufficient balance")]
    InsufficientBalance,
    #[error("No Money")]
    NoMoney,
}

impl ResponseError for XProtocolError {
    fn status_code(&self) -> StatusCode {
        StatusCode::OK
    }

    fn error_response(&self) -> HttpResponse {
        let status_code = match *self {
            Self::InsufficientStorage => StatusCode::INSUFFICIENT_STORAGE,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::BadRequest => StatusCode::BAD_REQUEST,
            Self::MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,
            Self::ExpectationFailed => StatusCode::EXPECTATION_FAILED,
            Self::InternalServerError => StatusCode::INTERNAL_SERVER_ERROR,
            Self::ChainIdNotFound => StatusCode::EXPECTATION_FAILED,
            Self::Unknown => StatusCode::BAD_REQUEST,
            Self::Overflow => StatusCode::BAD_REQUEST,
            Self::InsufficientBalance => StatusCode::BAD_REQUEST,
            Self::NoMoney => StatusCode::BAD_REQUEST,
        };
        HttpResponse::build(status_code).json(ErrorResponse::new(status_code, self.to_string()))
    }
}
