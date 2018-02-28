use users::models::User;
use types::*;
use rocket_contrib::Json;
use db::DbConnection;
use diesel::prelude::*;
use diesel::{debug_query, select};
use diesel::result::{DatabaseErrorKind, Error};
use std::borrow::Cow;

#[derive(Debug, Serialize)]
pub struct ProfileResponse<'a> {
    profile: Profile<'a>,
}

#[derive(Debug, Serialize)]
pub struct Profile<'a> {
    pub username: Cow<'a, str>,
    pub bio: Option<Cow<'a, str>>,
    pub image: Option<Cow<'a, str>>,
    pub following: bool,
}

#[get("/profiles/<name>", format = "application/json")]
pub fn profile(
    connection: DbConnection,
    current_user: Option<User>,
    name: String,
) -> ApiResult<ProfileResponse<'static>> {
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
        username: Cow::Owned(user.username),
        image: user.image.map(|v| Cow::Owned(v)),
        bio: user.bio.map(|v| Cow::Owned(v)),
        following: following,
    };

    Ok(Json(ProfileResponse { profile }))
}

#[delete("/profiles/<name>/follow", format = "application/json")]
pub fn unfollow(
    connection: DbConnection,
    current_user: Result<User, ApiError>,
    name: String,
) -> ApiResult<ProfileResponse<'static>> {
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
        username: Cow::Owned(follow.username),
        bio: follow.bio.map(|v| Cow::Owned(v)),
        image: follow.image.map(|v| Cow::Owned(v)),
        following: false,
    };

    Ok(Json(ProfileResponse { profile: profile }))
}

#[post("/profiles/<name>/follow", format = "application/json")]
pub fn follow(
    connection: DbConnection,
    current_user: Result<User, ApiError>,
    name: String,
) -> ApiResult<ProfileResponse<'static>> {
    use db::schema::followers::dsl::*;
    use diesel::insert_into;
    use diesel::pg::Pg;

    let current = current_user?;
    let follow = User::load_by_name(&name, &connection)?;
    insert_into(followers)
        .values((user_id.eq(&current.id), follower_id.eq(&follow.id)))
        .on_conflict((user_id, follower_id))
        .do_nothing()
        .execute(&*connection)?;
    let profile = Profile {
        username: Cow::Owned(follow.username),
        bio: follow.bio.map(|v| Cow::Owned(v)),
        image: follow.image.map(|v| Cow::Owned(v)),
        following: true,
    };

    let resp = ProfileResponse { profile: profile };
    Ok(Json(resp))
}
