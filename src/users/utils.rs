use crypto::sha2::Sha256;
use std::io::Result as IoResult;
use types::{ApiError, ValidationError};
use super::models::User;
use diesel::PgConnection;
use regex::Regex;
use diesel::dsl::exists;
use diesel::select;
use diesel::prelude::*;

use jwt::{Header, Registered, Token};

lazy_static!{
    static ref EMAIL_RE: Regex = {
        let pattern = r"\A[a-z0-9!#$%&'*+/=?^_`{|}~-]+(?:\.[a-z0-9!#$%&'*+/=?^_`{|}~-]+)*@(?:[a-z0-9](?:[a-z0-9-]*[a-z0-9])?\.)+[a-z0-9](?:[a-z0-9-]*[a-z0-9])?\z";
        Regex::new(pattern).unwrap()
    };
}

pub fn validate_email_re(email: &str) -> Result<(), ValidationError> {
    if !EMAIL_RE.is_match(email) {
        Err(ValidationError::from(
            "email",
            format!("Invalid email: {}", email),
        ))
    } else {
        Ok(())
    }
}

pub fn validate_username_re(username: &str) -> Result<(), ValidationError> {
    if username.len() < 3 {
        Err(ValidationError::from(
            "email",
            format!("username to small: {}", username),
        ))
    } else {
        Ok(())
    }
}

pub fn validate_email(email_to_validate: &str, connection: &PgConnection) -> Result<(), ApiError> {
    use db::schema::users::dsl::*;
    let mut errors = ValidationError::default();
    if !EMAIL_RE.is_match(email_to_validate) {
        errors.add_error("email", format!("Invalid email: {}", email_to_validate));
    }

    let email_exists =
        select(exists(users.filter(email.eq(email_to_validate)))).get_result::<bool>(connection)?;
    if email_exists {
        errors.add_error("email", "Email allready exists");
    }
    if errors.len() > 0 {
        Err(errors.into())
    } else {
        Ok(())
    }
}

pub fn validate_password(password: &str) -> Result<(), ValidationError> {
    if password.len() < 5 {
        let e = ValidationError::from("password", "Password to short");
        Err(e)
    } else {
        Ok(())
    }
}

/*pub fn validate_username(
    username_to_validate: &str,
    connection: &PgConnection,
) -> Result<(), ApiError> {
    use db::schema::users::dsl::*;
    let mut errors = ValidationError::default();
    let username_exists = select(exists(users.filter(username.eq(username_to_validate))))
        .get_result::<bool>(connection)?;

    if username_exists {
        errors.add_error("username", "username allready exists");
    }

    if errors.len() > 0 {
        Err(errors.into())
    } else {
        Ok(())
    }
}*/
