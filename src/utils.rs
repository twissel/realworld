use serde_json::{self, Value};
use rocket::response::content::Json;
use rocket::response::{Responder, Response};
use rocket::http::Status;
use rocket::request::Request;

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
