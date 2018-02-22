use users::models::User;
use types::*;
use rocket_contrib::Json;
use db::DbConnection;
use diesel::prelude::*;
use diesel::{debug_query, select};
use diesel::result::{DatabaseErrorKind, Error};
use db::schema::{articles, users};
use chrono::NaiveDateTime;

#[derive(Identifiable, Queryable, Associations, PartialEq, Debug)]
#[belongs_to(User, foreign_key = "articles_users_id_fk")]
#[table_name = "articles"]
#[allow(non_snake_case)]
pub struct Article {
    id: i32,
    slug: String,
    title: String,
    description: String,
    body: String,
    tagList: Vec<String>,
    createdAt: NaiveDateTime,
    updatedAt: Option<NaiveDateTime>,
}

#[post("/articles", format = "application/json")]
pub fn create(user: Result<User, ApiError>) {}
