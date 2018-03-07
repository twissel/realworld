use rocket_contrib::Json;
use rocket_contrib::Value;
pub mod models;
use types::{ApiError, ApiResult, Validate, ValidationError};
use rocket::response::{Responder, Response};
use rocket::{Request, State};
use rocket::http::Status;
use std::collections::HashMap;
use db::{DbConnection, Pool};
use diesel::prelude::*;
use diesel::dsl::exists;
use diesel::{debug_query, select, update as diesel_update};
use std::convert::From;
use db::schema::users;
use diesel::insert_into;
use std::ops::Deref;
use rocket::Route;
use rocket::outcome::IntoOutcome;
use diesel::associations::HasTable;
use rocket::request::{self, FromRequest};
use rocket::Outcome;

mod utils;

pub type CurrentUser = Result<models::User, ApiError>;

use self::utils::*;
#[derive(Debug, Deserialize)]
struct RegistrationDetails {
    username: String,
    email: String,
    password: String,
}

#[derive(Debug, Deserialize)]
pub struct Registration {
    user: RegistrationDetails,
}

impl Validate for Registration {
    type Error = ApiError;
    fn validate(self, connection: &PgConnection) -> Result<Self, Self::Error> {
        use db::schema::users::dsl::*;
        let mut errors = ValidationError::default();

        let is_valid_email = validate_email(&self.user.email, connection);
        match is_valid_email {
            Ok(_) => {}
            Err(e) => match e {
                ApiError::Validation(e) => {
                    errors.merge(e);
                }
                other => return Err(other),
            },
        }

        let is_valid_password = validate_password(&self.user.password);
        match is_valid_password {
            Ok(_) => {}
            Err(e) => errors.merge(e),
        }

        let username_exists = select(exists(users.filter(username.eq(&self.user.username))))
            .get_result::<bool>(connection)?;

        if username_exists {
            errors.add_error("username", "username allready exists");
        }

        if errors.len() > 0 {
            Err(errors.into())
        } else {
            Ok(self)
        }
    }
}

#[post("/", format = "application/json", data = "<registration>")]
pub fn register(connection: DbConnection, registration: Json<Registration>) -> ApiResult<Value> {
    use db::schema::users::dsl::*;

    let registration = registration.validate(&connection)?;
    let new_user = models::NewUser {
        username: registration.user.username.clone(),
        email: registration.user.email.clone(),
        token: models::User::make_password(&registration.user.password)?,
    };

    let mut user = insert_into(users)
        .values(&new_user)
        .get_result::<models::User>(&*connection)?;
    user.token = user.token()?;
    Ok(Json(json!({ "user": user })))
}

#[derive(Debug, Deserialize)]
struct LoginDetails {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct Login {
    user: LoginDetails,
}

impl<'a, 'r> FromRequest<'a, 'r> for models::User {
    type Error = ApiError;
    fn from_request(request: &'a Request<'r>) -> request::Outcome<Self, Self::Error> {
        let headers = request.headers();
        let token_header = headers.get_one("Authorization");
        if let Some(token_header) = token_header {
            let token = str::replace(token_header, "Token ", "");
            let connection = DbConnection::from_request(request);
            match connection {
                Outcome::Success(connection) => {
                    let user = models::User::load_from_token(&token, &connection);
                    match user {
                        Ok(user) => Outcome::Success(user),
                        Err(e) => match e {
                            ApiError::Validation(_) => Outcome::Failure((Status::raw(422), e)),
                            _ => Outcome::Failure((Status::ServiceUnavailable, ApiError::Internal)),
                        },
                    }
                }
                _ => Outcome::Failure((Status::ServiceUnavailable, ApiError::Internal)),
            }
        } else {
            Outcome::Failure((Status::raw(401), ApiError::Unauthorized))
        }
    }
}

#[post("/login", format = "application/json", data = "<login>")]
pub fn login(connection: DbConnection, login: Json<Login>) -> ApiResult<Value> {
    use db::schema::users::dsl::*;
    let mut user = users
        .filter(email.eq(&login.user.email))
        .first::<models::User>(&*connection)?;
    let password_is_valid = user.verify_password(&login.user.password)?;
    match password_is_valid {
        true => {
            user.token = user.token()?;
            Ok(Json(json!({ "user": user })))
        }
        false => {
            let mut error = ValidationError::default();
            error.add_error("password", "Invalid password");
            Err(error.into())
        }
    }
}

#[get("/user", format = "application/json")]
pub fn current(user: Result<models::User, ApiError>) -> ApiResult<Value> {
    let user = json!({"user": 
        user?
    });
    Ok(Json(user))
}

#[derive(Debug, Deserialize, AsChangeset)]
#[table_name = "users"]
pub struct UpdateUser {
    pub username: Option<String>,
    pub email: Option<String>,
    #[column_name = "token"]
    pub password: Option<String>,
    pub image: Option<String>,
    pub bio: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Update {
    pub user: UpdateUser,
}

#[put("/user", format = "application/json", data = "<update>")]
pub fn update(
    curent_user: CurrentUser,
    connection: DbConnection,
    update: Json<Update>,
) -> ApiResult<Value> {
    use db::schema::users::dsl::*;

    let mut user = curent_user?;
    let mut error = ValidationError::default();
    let update = update.into_inner();

    user.bio = update.user.bio;
    user.image = update.user.image;

    if let Some(new_email) = update.user.email {
        let is_valid = validate_email_re(&new_email);
        match is_valid {
            Err(e) => {
                error.merge(e);
            }
            Ok(_) => {
                user.email = new_email;
            }
        }

        let expr = users.filter(email.eq(&user.email)).filter(id.ne(&user.id));
        let email_exists = select(exists(expr)).get_result::<bool>(&*connection)?;
        if email_exists {
            error.add_error("email", format!("Email already chosen: {}", &user.email));
        }
    }

    if let Some(new_username) = update.user.username {
        let is_valid = validate_username_re(&new_username);
        match is_valid {
            Err(e) => {
                error.merge(e);
            }
            Ok(_) => {
                user.username = new_username;
            }
        }
        let expr = users
            .filter(username.eq(&user.username))
            .filter(id.ne(user.id));
        let username_exists = select(exists(expr)).get_result::<bool>(&*connection)?;
        if username_exists {
            error.add_error(
                "username",
                format!("Username already chosen: {}", user.username),
            );
        }
    }

    if let Some(new_password) = update.user.password {
        let is_valid = validate_password(&new_password);
        match is_valid {
            Err(e) => {
                error.merge(e);
            }
            _ => {
                user.new_password(&new_password)?;
            }
        }
    }

    if !error.empty() {
        Err(error.into())
    } else {
        diesel_update(&user).set(&user).execute(&*connection)?;
        user.token = user.token()?;
        Ok(Json(json!({ "user": user })))
    }
}


