#![feature(plugin)]
#![feature(custom_derive)]
#![plugin(rocket_codegen)]
#![plugin(dotenv_macros)]
#![allow(unused_imports)]
#![feature(underscore_lifetimes)]
#![feature(entry_or_default)]

extern crate dotenv;
#[macro_use]
extern crate dotenv_codegen;
extern crate rocket;

#[macro_use]
extern crate error_chain;

#[macro_use]
extern crate diesel;
extern crate r2d2;
extern crate r2d2_diesel;

extern crate chrono;
extern crate crypto;
extern crate jwt;
#[macro_use]
extern crate lazy_static;
extern crate regex;
#[macro_use]
extern crate rocket_contrib;
#[macro_use]
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

//extern crate validator;
//#[macro_use]
//#extern crate validator_derive;

extern crate slug;

mod db;
mod users;
mod types;
mod utils;
mod profile;
mod article;

use rocket::request::Request;
use rocket::Error;
use rocket::response::content;

#[error(422)]
fn handle_422(_req: &Request, _e: Error) -> content::Json<String> {
    let json = json!({
        "errors": [
            "entity not found"
        ]
    });
    content::Json(json.to_string())
}

#[error(404)]
fn not_found(_req: &Request) -> content::Json<String> {
    let json = json!({
        "errors": [
            "entity not found"
        ]
    });
    content::Json(json.to_string())
}

fn main() {
    let pool = db::init_pool().expect("Failed to create database pool");
    rocket::ignite()
        .manage(pool)
        .mount("/api/users", routes!(users::register, users::login,))
        .mount("/api", routes!(users::current, users::update))
        .mount(
            "/api",
            routes!(profile::profile, profile::follow, profile::unfollow),
        )
        .mount(
            "/api/articles",
            routes!(
                article::get,
                article::create,
                article::favorite,
                article::update
            ),
        )
        .catch(errors![not_found, handle_422])
        .launch();
}
