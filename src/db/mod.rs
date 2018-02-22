use diesel::prelude::*;
use diesel::pg::PgConnection;
use dotenv::dotenv;
use std::env;
use r2d2_diesel::ConnectionManager;
use r2d2;
use diesel::result::Error as DieselError;
use std::ops::Deref;
use rocket::http::Status;
use rocket::request::{self, FromRequest};
use rocket::{Outcome, Request, State};

pub mod schema;

// An alias to the type for a pool of Diesel Postgres connections.
pub type Pool = r2d2::Pool<ConnectionManager<PgConnection>>;

pub struct DbConnection(pub r2d2::PooledConnection<ConnectionManager<PgConnection>>);

error_chain! {
    foreign_links {
        Var(::std::env::VarError);
        R2D2(r2d2::Error);
        Diesel(DieselError);
    }
}

// Attempts to retrieve a single connection from the managed database pool. If
/// no pool is currently managed, fails with an `InternalServerError` status. If
/// no connections are available, fails with a `ServiceUnavailable` status.
impl<'a, 'r> FromRequest<'a, 'r> for DbConnection {
    type Error = ();

    fn from_request(request: &'a Request<'r>) -> request::Outcome<DbConnection, ()> {
        let pool = request.guard::<State<Pool>>()?;
        match pool.get() {
            Ok(conn) => Outcome::Success(DbConnection(conn)),
            Err(_) => Outcome::Failure((Status::ServiceUnavailable, ())),
        }
    }
}

// For the convenience of using an &DbConnection as an &PgConnection.
impl Deref for DbConnection {
    type Target = PgConnection;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub fn init_pool() -> Result<Pool> {
    dotenv().ok();
    let database_url = env::var("DATABASE_URL")?;
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    Ok(Pool::new(manager)?)
}

pub trait TryLoadById
where
    Self: Sized,
{
    fn try_load_by_id(id: i32) -> Result<Self>;
}
