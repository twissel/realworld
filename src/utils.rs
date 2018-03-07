use serde_json::{self, Value};
use rocket::response::content::Json;
use rocket::response::{Responder, Response};
use rocket::http::Status;
use rocket::request::Request;
use chrono::{DateTime, SecondsFormat, Utc};
use serde::Serializer;

pub fn try_respond(
    req: &Request,
    json: &Value,
    status: Status,
) -> Result<Response<'static>, Status> {
    let as_json = serde_json::to_string(&json);
    match as_json {
        Ok(json) => Json(json)
            .respond_to(req)
            .and_then(|resp| Response::build_from(resp).status(status).ok()),

        Err(_) => Err(Status::raw(500)),
    }
}

pub fn serialize_date<S>(date: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let s = date.to_rfc3339_opts(SecondsFormat::Millis, true);
    serializer.serialize_str(&s)
}
