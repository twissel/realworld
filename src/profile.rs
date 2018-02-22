use users::models::User;
use types::*;
use rocket_contrib::Json;
use db::DbConnection;
use diesel::prelude::*;
use diesel::{debug_query, select};
use diesel::result::{DatabaseErrorKind, Error};

#[derive(Debug, Serialize)]
pub struct ProfileResponse {
    profile: Profile,
}

#[derive(Debug, Serialize)]
pub struct Profile {
    username: String,
    bio: Option<String>,
    image: Option<String>,
    following: bool,
}

#[get("/profiles/<name>", format = "application/json")]
pub fn profile(
    connection: DbConnection,
    current_user: Option<User>,
    name: String,
) -> ApiResult<ProfileResponse> {
    use db::schema::users::dsl::*;
    use db::schema::followers::dsl::*;
    use diesel::dsl::exists;

    let user = User::load_by_name(&name, &connection)?;
    let following = match current_user {
        Some(current) => {
            let query = select(exists(
                followers
                    .filter(user_id.eq(&current.id))
                    .filter(follower_id.eq(&user.id)),
            ));
            query.get_result::<bool>(&*connection)?
        }
        None => false,
    };

    let profile = Profile {
        username: user.username,
        image: user.image,
        bio: user.bio,
        following: following,
    };

    Ok(Json(ProfileResponse { profile }))
}

#[delete("/profiles/<name>/follow", format = "application/json")]
pub fn unfollow(
    connection: DbConnection,
    current_user: Result<User, ApiError>,
    name: String,
) -> ApiResult<ProfileResponse> {
    use db::schema::followers::dsl::*;
    use diesel::insert_into;
    use diesel::pg::Pg;
    use diesel::delete;

    let current = current_user?;
    let follow = User::load_by_name(&name, &connection)?;
    delete(
        followers
            .filter(user_id.eq(&current.id))
            .filter(follower_id.eq(&follow.id)),
    ).execute(&*connection)?;
    let profile = Profile {
        username: follow.username,
        bio: follow.bio,
        image: follow.image,
        following: false,
    };

    Ok(Json(ProfileResponse { profile: profile }))
}

#[post("/profiles/<name>/follow", format = "application/json")]
pub fn follow(
    connection: DbConnection,
    current_user: Result<User, ApiError>,
    name: String,
) -> ApiResult<ProfileResponse> {
    use db::schema::followers::dsl::*;
    use diesel::insert_into;
    use diesel::pg::Pg;

    let current = current_user?;
    let follow = User::load_by_name(&name, &connection)?;
    let insert = insert_into(followers)
        .values((user_id.eq(&current.id), follower_id.eq(&follow.id)))
        .execute(&*connection);
    let profile = Profile {
        username: follow.username,
        bio: follow.bio,
        image: follow.image,
        following: true,
    };

    let resp = ProfileResponse { profile: profile };

    match insert {
        Err(e) => match e {
            Error::DatabaseError(kind, _) => match kind {
                DatabaseErrorKind::UniqueViolation => Ok(Json(resp)),
                _ => Err(ApiError::Internal),
            },
            _ => Ok(Json(resp)),
        },
        Ok(_) => Ok(Json(resp)),
    }
}
