use rocket_contrib::json::*;
use rocket::data::{self, Data, FromData};
use rocket::outcome::{IntoOutcome, Outcome};
use rocket::request::Request;
use serde::de::DeserializeOwned;
use std::ops::Deref;
use rocket::http::{ContentType, Status};
use std::fmt::Debug;
use rocket::response::{Responder, Response};
use rocket::response::content;
use std::collections::HashMap;
use serde::Serialize;
use serde_json;
use db::DbConnection;
use diesel::result::Error as DieselError;
use std::io::Error as IoError;
use diesel::PgConnection;
use utils::try_respond;
use rocket::request::Outcome as RequestOutcome;

pub trait Validate
where
    Self: Sized,
{
    type Error;
    fn validate(self, connection: &PgConnection) -> Result<Self, Self::Error>;
}

#[derive(Debug)]
pub enum ApiError {
    Diesel(DieselError),
    Validation(ValidationError),
    Internal,
    Unauthorized,
}

impl From<DieselError> for ApiError {
    fn from(err: DieselError) -> ApiError {
        ApiError::Diesel(err)
    }
}

impl From<ValidationError> for ApiError {
    fn from(err: ValidationError) -> ApiError {
        ApiError::Validation(err)
    }
}

impl From<IoError> for ApiError {
    fn from(_: IoError) -> ApiError {
        ApiError::Internal
    }
}

pub type ApiResult<T> = Result<Json<T>, ApiError>;

#[derive(Debug, Serialize, Default)]
pub struct ValidationError(HashMap<String, Vec<String>>);

impl ValidationError {
    pub fn add_error<K: Into<String>, V: Into<String>>(&mut self, key: K, val: V) {
        let entry = self.0.entry(key.into()).or_insert(Vec::default());
        entry.push(val.into());
    }

    pub fn from<K: Into<String>, V: Into<String>>(key: K, val: V) -> Self {
        let mut error = ValidationError::default();
        error.add_error(key, val);
        error
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn merge(&mut self, other: ValidationError) {
        for (key, errors) in other.0.into_iter() {
            let entry = self.0.entry(key).or_default();
            entry.extend(errors);
        }
    }

    pub fn empty(&self) -> bool {
        self.len() == 0
    }
}

impl<'r> Responder<'r> for ApiError {
    fn respond_to(self, req: &Request) -> Result<Response<'r>, Status> {
        match self {
            ApiError::Diesel(error) => match error {
                DieselError::NotFound => Err(Status::raw(404)),
                _ => Err(Status::raw(500)),
            },

            ApiError::Validation(error) => {
                let body = json!({ "errors": error });
                try_respond(req, &body, Status::raw(422))
            }

            ApiError::Unauthorized => {
                let body = json!({ "errors": {
                    "status": "401 Unauthorized"
                }});
                try_respond(req, &body, Status::raw(422))
            }
            _ => Err(Status::raw(500)),
        }
    }
}

impl<T> Validate for Json<T>
where
    T: Validate,
{
    type Error = <T as Validate>::Error;
    fn validate(self, connection: &PgConnection) -> Result<Self, Self::Error> {
        let inner = self.0;
        let validated = inner.validate(connection)?;
        Ok(Json(validated))
    }
}
