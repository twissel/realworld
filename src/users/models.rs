use diesel::prelude::*;
use db::{DbConnection, TryLoadById};
use db::schema::users;
use crypto::pbkdf2::*;
use crypto::sha2::Sha256;
use std::io::Result as IoResult;
use types::{ApiError, ValidationError};
use jwt::{Header, Registered, Token};

#[derive(Debug, Queryable, Identifiable, Serialize, AsChangeset)]
pub struct User {
    #[serde(skip_serializing)]
    pub id: i32,
    pub username: String,
    pub token: String,
    pub email: String,
    pub bio: Option<String>,
    pub image: Option<String>,
}

impl User {
    pub fn make_password(password: &String) -> IoResult<String> {
        pbkdf2_simple(password, 1000)
    }

    pub fn new_password(&mut self, password: &String) -> IoResult<()> {
        self.token = pbkdf2_simple(password, 1000)?;
        Ok(())
    }

    pub fn verify_password(&self, password_to_verify: &String) -> Result<bool, ApiError> {
        let check = pbkdf2_check(password_to_verify, &self.token);
        check.map_err(|_| ApiError::Internal)
    }

    pub fn token(&self) -> Result<String, ApiError> {
        let header = Header::default();
        let claims = Registered {
            iss: Some(self.email.clone()),
            sub: Some(self.id.to_string()),
            ..Default::default()
        };
        let token = Token::new(header, claims);
        token
            .signed(self.token.as_bytes(), Sha256::new())
            .map_err(|_| ApiError::Internal)
    }

    pub fn verify_token(&self, token: &Token<Header, Registered>) -> bool {
        token.verify(&self.token.as_bytes(), Sha256::new())
    }

    pub fn load_from_token(jwt_token: &str, connection: &PgConnection) -> Result<User, ApiError> {
        use db::schema::users::dsl::*;
        let jwt_token = Token::<Header, Registered>::parse(jwt_token);
        let mut e = ValidationError::default();
        match jwt_token {
            Ok(jwt_token) => {
                let sub = &jwt_token.claims.sub;
                match sub {
                    &None => Err(ValidationError::from("token", "Invalid jwt token").into()),
                    &Some(ref user_id) => match &jwt_token.claims.iss {
                        &None => Err(ValidationError::from("token", "Invalid jwt token").into()),
                        &Some(ref user_email) => {
                            let user_id = user_id.parse::<i32>().map_err(|_| {
                                ApiError::Validation(ValidationError::from(
                                    "token",
                                    "Invalid jwt token",
                                ))
                            })?;

                            let user = users
                                .filter(id.eq(user_id))
                                .filter(email.eq(user_email))
                                .get_result::<User>(connection)?;
                            Ok(user)
                        }
                    },
                }
            }
            Err(_) => {
                e.add_error("token", "Invalid jwt token");
                Err(e.into())
            }
        }
    }

    pub fn load_by_name(name: &str, connection: &PgConnection) -> Result<User, ApiError> {
        use db::schema::users::dsl::*;
        users
            .filter(username.eq(&name))
            .get_result::<User>(connection)
            .map_err(|e| e.into())
    }
}

#[derive(Deserialize, Insertable, Serialize)]
#[table_name = "users"]
pub struct NewUser {
    pub username: String,
    pub token: String,
    pub email: String,
}
