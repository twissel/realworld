use chrono::{DateTime, Utc};
use db::schema::{comments, followers, users};
use db::DbConnection;
use users::models::User;
use users::CurrentUser;
use article::Article;
use rocket_contrib::Json;
use types::{ApiError, ApiResult};
use diesel::insert_into;
use diesel::prelude::*;
use utils::serialize_date;
use serde::de::Deserialize;
use std::fmt::Debug;
use profile::Profile;
use diesel::BelongingToDsl;
use diesel::{delete as diesel_delete, select};
use diesel::dsl::{any, exists, sql};
use diesel::sql_types::Integer;
use std::collections::HashMap;
use std::convert::From;

allow_tables_to_appear_in_same_query!(comments, users);

#[derive(Debug, Serialize, Associations, PartialEq, AsChangeset, Identifiable, Queryable)]
#[belongs_to(Article)]
pub struct Comment {
    id: i32,
    article_id: i32,
    user_id: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    body: String,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CommentView<'r> {
    id: i32,
    #[serde(serialize_with = "serialize_date")]
    created_at: DateTime<Utc>,
    #[serde(serialize_with = "serialize_date")]
    updated_at: DateTime<Utc>,
    body: String,
    author: Profile<'r>,
}

impl<'r> From<(Comment, Profile<'r>)> for CommentView<'r> {
    fn from(comment_and_profile: (Comment, Profile<'r>)) -> Self {
        let comment = comment_and_profile.0;
        let profile = comment_and_profile.1;
        CommentView {
            id: comment.id,
            author: profile,
            created_at: comment.created_at,
            updated_at: comment.updated_at,
            body: comment.body,
        }
    }
}

#[derive(Deserialize, Insertable, Serialize)]
#[table_name = "comments"]
pub struct NewComment {
    #[serde(skip_serializing)]
    article_id: i32,
    #[serde(skip_serializing)]
    user_id: i32,
    #[serde(serialize_with = "serialize_date")]
    created_at: DateTime<Utc>,
    #[serde(serialize_with = "serialize_date")]
    updated_at: DateTime<Utc>,
    body: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CommentBody {
    body: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CommentContainer<T> {
    comment: T,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CommentsContainer<T> {
    comments: T,
}

#[post("/<slug>/comments", data = "<details>", format = "application/json")]
pub fn add(
    conn: DbConnection,
    user: CurrentUser,
    slug: String,
    details: Json<CommentContainer<CommentBody>>,
) -> ApiResult<CommentContainer<CommentView<'static>>> {
    let details = details.into_inner();
    let article = Article::load_by_slug(&slug, &*conn)?;
    let user = user?;
    let now = Utc::now();
    let new_comment = NewComment {
        article_id: article.id,
        user_id: user.id,
        created_at: now.clone(),
        updated_at: now,
        body: details.comment.body,
    };

    let comment = insert_into(comments::table)
        .values(&new_comment)
        .get_result::<Comment>(&*conn)?;

    let profile = user.profile(false);

    let container = CommentContainer {
        comment: (comment, profile).into(),
    };
    Ok(Json(container))
}

#[get("/<slug>/comments", format = "application/json")]
fn get(
    conn: DbConnection,
    user: CurrentUser,
    slug: String,
) -> ApiResult<CommentsContainer<Vec<CommentView<'static>>>> {
    let article = Article::load_by_slug(&slug, &conn)?;
    let data = Comment::belonging_to(&article)
        .inner_join(users::table.on(comments::user_id.eq(users::id)))
        .get_results::<(Comment, User)>(&*conn)?;

    match user {
        Ok(user) => {
            let authors = data.iter().map(|elem| elem.1.id).collect::<Vec<i32>>();
            let follows = exists(
                followers::table.select(sql::<Integer>("1")).filter(
                    followers::follower_id
                        .eq(user.id)
                        .and(users::id.eq(followers::user_id)),
                ),
            );

            let mut follows = users::table
                .select((users::id, follows))
                .filter(users::id.eq(any(authors)))
                .get_results::<(i32, bool)>(&*conn)?
                .into_iter()
                .collect::<HashMap<_, _>>();
            let comments = data.into_iter().map(|elem| {
                let comment = elem.0;
                let author = elem.1;
                let follows_user = follows.remove(&user.id).unwrap_or(false);
                let profile = author.profile(follows_user);
                (comment, profile).into()
            });
            Ok(Json(CommentsContainer {
                comments: comments.collect(),
            }))
        }
        Err(_) => {
            let comments = data.into_iter().map(|elem| {
                let comment = elem.0;
                let author = elem.1;
                let profile = author.profile(false);
                (comment, profile).into()
            });
            Ok(Json(CommentsContainer {
                comments: comments.collect(),
            }))
        }
    }
}

#[delete("/<_slug>/comments/<id>", format = "application/json")]
fn delete(conn: DbConnection, user: CurrentUser, _slug: String, id: i32) -> ApiResult<()> {
    let user = user?;
    let comment = comments::table.find(id).first::<Comment>(&*conn)?;
    if comment.user_id != user.id {
        return Err(ApiError::Forbidden);
    }
    diesel_delete(&comment).execute(&*conn)?;
    Ok(Json(()))
}
