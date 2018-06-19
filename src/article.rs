use chrono::format::{Fixed, Item, Numeric, Pad};
use chrono::{DateTime, Local, NaiveDateTime, Utc};
use db::schema::{articles, favorites, followers, users};
use db::DbConnection;
use diesel::associations::HasTable;
use diesel::dsl::sql;
use diesel::dsl::{count, count_star, exists, Eq, Filter, Limit, Offset};
use diesel::expression::{AsExpression, BoxableExpression, Expression, SelectableExpression};
use diesel::pg::expression::dsl::any;
use diesel::pg::types::sql_types::Array;
use diesel::pg::Pg;
use diesel::prelude::*;
use diesel::query_dsl;
use diesel::result::{DatabaseErrorKind, Error};
use diesel::sql_types::{BigInt, Bool, Integer, Nullable, Text, Timestamptz};
use diesel::PgArrayExpressionMethods;
use diesel::{debug_query, delete as diesel_delete, select};
use diesel::{insert_into, sql_query, update as diesel_update};
use profile::Profile;
use regex::Regex;
use rocket_contrib::Json;
use serde::ser::{Serialize, SerializeStruct, Serializer};
use slug::slugify;
use std::borrow::Cow;
use std::collections::HashMap;
use types::*;
use users::models::User;
use users::CurrentUser;
use utils;

allow_tables_to_appear_in_same_query!(users, articles);
allow_tables_to_appear_in_same_query!(users, favorites);
allow_tables_to_appear_in_same_query!(users, followers);
allow_tables_to_appear_in_same_query!(articles, favorites);

#[derive(Identifiable, Queryable, Associations, PartialEq, Debug, Deserialize, Serialize,
         AsChangeset)]
#[belongs_to(User, foreign_key = "author_id")]
#[table_name = "articles"]
#[serde(rename_all = "camelCase")]
pub struct Article {
    #[serde(skip_serializing)]
    pub id: i32,

    #[serde(skip_serializing)]
    pub author_id: i32,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub body: String,
    pub tag_list: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Article {
    pub fn load_by_slug(slug_: &str, connection: &PgConnection) -> Result<Article, ApiError> {
        use db::schema::articles::dsl::*;
        articles
            .filter(slug.eq(&slug_))
            .get_result::<Article>(connection)
            .map_err(|e| e.into())
    }

    pub fn by_slug<'r>(article_slug: &'r str) -> BySlug<'r> {
        use db::schema::articles::dsl::*;
        let condition = slug.eq(article_slug);
        articles.filter(condition)
    }

    pub fn get_favorites_count(&self, conn: &PgConnection) -> Result<i64, ApiError> {
        favorites::table
            .select(count_star())
            .filter(favorites::article_id.eq(&self.id))
            .get_result::<i64>(conn)
            .map_err(|e| e.into())
    }

    pub fn is_favorited_by(&self, user: &User, conn: &PgConnection) -> Result<bool, ApiError> {
        select(exists(
            favorites::table.filter(
                favorites::article_id
                    .eq(&self.id)
                    .and(favorites::user_id.eq(&user.id)),
            ),
        )).get_result::<bool>(conn)
            .map_err(|e| e.into())
    }
}

#[derive(Debug, Serialize)]
pub struct RichArticleResponse<'r> {
    article: RichArticle<'r>,
}

#[derive(Debug, QueryableByName, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RichArticle<'a> {
    #[serde(skip_serializing)]
    #[sql_type = "Integer"]
    id: i32,
    #[sql_type = "Text"]
    slug: String,
    #[sql_type = "Text"]
    title: String,
    #[sql_type = "Text"]
    description: String,
    #[sql_type = "Text"]
    body: String,
    #[sql_type = "Nullable<Array<Text>>"]
    #[column_name = "tag_list"]
    tag_list: Option<Vec<String>>,
    #[sql_type = "Timestamptz"]
    #[column_name = "created_at"]
    #[serde(serialize_with = "utils::serialize_date")]
    created_at: DateTime<Utc>,
    #[sql_type = "Timestamptz"]
    #[serde(serialize_with = "utils::serialize_date")]
    #[column_name = "updated_at"]
    updated_at: DateTime<Utc>,
    #[sql_type = "BigInt"]
    favorites_count: i64,

    #[sql_type = "Bool"]
    favorited: bool,
    #[diesel(embed)]
    author: Profile<'a>,
}

type WithSlug<'a> = Eq<articles::slug, &'a str>;
type BySlug<'a> = Filter<articles::table, WithSlug<'a>>;

impl<'a> RichArticle<'a> {
    fn from(
        article: Article,
        author: Profile<'a>,
        favorites_count: Option<i64>,
        favorited: bool,
    ) -> Self {
        let favorites_count = match favorites_count {
            Some(c) => c,
            None => 0,
        };
        RichArticle {
            id: article.id,
            slug: article.slug,
            title: article.title,
            description: article.description,
            body: article.body,
            tag_list: Some(article.tag_list),
            created_at: article.created_at,
            updated_at: article.updated_at,
            favorites_count: favorites_count,
            favorited: favorited,
            author: author,
        }
    }
}

#[derive(Deserialize, Insertable, Serialize)]
#[table_name = "articles"]
pub struct NewArticle {
    author_id: i32,
    slug: String,
    title: String,
    description: String,
    body: String,
    tag_list: Vec<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
pub struct ArticleDetails {
    title: String,
    description: String,
    body: String,
    #[serde(default)]
    #[serde(rename = "tagList")]
    tag_list: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateArticle {
    article: ArticleDetails,
}

impl Validate for CreateArticle {
    type Error = ValidationError;
    fn validate(self, connection: &PgConnection) -> Result<Self, ValidationError> {
        match CreateOrUpdate::Create(&self).validate(connection) {
            Ok(_) => Ok(self),
            Err(e) => Err(e),
        }
    }
}

impl Validate for UpdateArticle {
    type Error = ValidationError;
    fn validate(self, connection: &PgConnection) -> Result<Self, ValidationError> {
        match CreateOrUpdate::Update(&self).validate(connection) {
            Ok(_) => Ok(self),
            Err(e) => Err(e),
        }
    }
}

pub enum CreateOrUpdate<'r> {
    Create(&'r CreateArticle),
    Update(&'r UpdateArticle),
}

fn add_error_if_empty(
    field: &str,
    error: &mut ValidationError,
    error_name: &str,
    error_description: &str,
) {
    if field.trim().len() == 0 {
        error.add_error(error_name, error_description);
    }
}

impl<'r> Validate for CreateOrUpdate<'r> {
    type Error = ValidationError;
    fn validate(self, _connection: &PgConnection) -> Result<Self, ValidationError> {
        let mut error = ValidationError::default();
        match self {
            CreateOrUpdate::Create(&CreateArticle { ref article }) => {
                add_error_if_empty(&article.body, &mut error, "body", "empty body");
                add_error_if_empty(&article.title, &mut error, "title", "empty title");
                add_error_if_empty(
                    &article.description,
                    &mut error,
                    "description",
                    "empty description",
                );
            }

            CreateOrUpdate::Update(&UpdateArticle { ref article }) => {
                if let Some(ref body) = article.body {
                    add_error_if_empty(body, &mut error, "body", "empty body");
                }
                if let Some(ref title) = article.title {
                    add_error_if_empty(title, &mut error, "title", "empty title");
                }
                if let Some(ref description) = article.description {
                    add_error_if_empty(description, &mut error, "description", "empty description");
                }
            }
        }

        if error.empty() {
            Ok(self)
        } else {
            Err(error)
        }
    }
}

#[post("/", format = "application/json", data = "<create>")]
pub fn create(
    connection: DbConnection,
    user: CurrentUser,
    create: Json<CreateArticle>,
) -> ApiResult<RichArticleResponse<'static>> {
    use db::schema::articles::dsl::*;
    let created = Utc::now();
    let create = create.validate(&*connection)?.into_inner();
    let user = user?;
    let new_article = NewArticle {
        author_id: user.id,
        slug: created.timestamp().to_string() + "-" + &slugify(&create.article.title),
        title: create.article.title,
        body: create.article.body,
        description: create.article.description,
        created_at: created,
        updated_at: created,
        tag_list: create.article.tag_list,
    };
    let article = insert_into(articles)
        .values(&new_article)
        .get_result::<Article>(&*connection)?;
    let author = user.profile(false);
    let rich_article = RichArticle::from(article, author, None, false);
    Ok(Json(RichArticleResponse {
        article: rich_article,
    }))
}

#[derive(Debug, Deserialize, AsChangeset)]
#[table_name = "articles"]
pub struct UpdateDetails {
    title: Option<String>,
    description: Option<String>,
    body: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateArticle {
    article: UpdateDetails,
}

#[put("/<slug>", format = "application/json", data = "<update>")]
pub fn update(
    slug: String,
    current_user: CurrentUser,
    update: Json<UpdateArticle>,
    connection: DbConnection,
) -> ApiResult<RichArticleResponse<'static>> {
    use db::schema::favorites::dsl::*;

    let current_user = current_user?;
    let mut article: Article = Article::by_slug(&slug).first(&*connection)?;
    if article.author_id != current_user.id {
        return Err(ApiError::Unauthorized);
    }

    let update = update.validate(&*connection)?.into_inner();
    if let Some(title) = update.article.title {
        article.title = title;
        article.slug = article.created_at.timestamp().to_string() + "-" + &slugify(&article.title);
    }

    if let Some(body) = update.article.body {
        article.body = body;
    }

    if let Some(description) = update.article.description {
        article.description = description;
    }

    article.updated_at = Utc::now();

    diesel_update(&article).set(&article).execute(&*connection)?;
    let favorited_count = article.get_favorites_count(&*connection)?;

    let favorited = article.is_favorited_by(&current_user, &*connection)?;

    let article = RichArticle::from(
        article,
        current_user.profile(false),
        Some(favorited_count),
        favorited,
    );
    Ok(Json(RichArticleResponse { article: article }))
}

#[get("/<slug_>", format = "application/json")]
pub fn get(
    slug_: String,
    connection: DbConnection,
    current_user: CurrentUser,
) -> ApiResult<RichArticleResponse<'static>> {
    let data = articles::table
        .inner_join(users::table.on(articles::author_id.eq(users::id)))
        .filter(articles::slug.eq(&slug_))
        .limit(1)
        .get_result::<(Article, User)>(&*connection)?;
    let article = data.0;
    let author = data.1;

    let mut favorited = false;
    let mut followed = false;

    let fav_count = article.get_favorites_count(&*connection)?;

    match current_user {
        Ok(user) => {
            favorited = article.is_favorited_by(&user, &*connection)?;

            followed = select(exists(
                followers::table.filter(
                    followers::follower_id
                        .eq(&user.id)
                        .and(followers::user_id.eq(&author.id)),
                ),
            )).get_result::<bool>(&*connection)?;
        }
        Err(e) => match e {
            ApiError::Unauthorized => {}
            e @ _ => return Err(e),
        },
    }

    let rich_article = RichArticle::from(
        article,
        author.profile(followed),
        Some(fav_count),
        favorited,
    );
    Ok(Json(RichArticleResponse {
        article: rich_article,
    }))
}

#[post("/<slug>/favorite", format = "application/json")]
pub fn favorite(
    slug: String,
    connection: DbConnection,
    current_user: CurrentUser,
) -> ApiResult<RichArticleResponse<'static>> {
    use db::schema::articles::dsl as articles_dsl;
    use db::schema::favorites::dsl::*;

    let current_user = current_user?;
    let fav_article_id = articles_dsl::articles
        .select(articles_dsl::id)
        .filter(articles_dsl::slug.eq(&slug))
        .first::<i32>(&*connection)?;

    insert_into(favorites)
        .values((&user_id.eq(current_user.id), &article_id.eq(fav_article_id)))
        .on_conflict((user_id, article_id))
        .do_nothing()
        .execute(&*connection)?;

    let article = Article::load_by_slug(&slug, &connection)?;
    let favorited = article.is_favorited_by(&current_user, &connection)?;
    let fav_count = article.get_favorites_count(&connection)?;
    let author = User::load_by_id(&article.author_id, &connection)?;
    let following = author.is_followed_by(&current_user, &connection)?;

    let article = RichArticle::from(
        article,
        author.profile(following),
        Some(fav_count),
        favorited,
    );

    Ok(Json(RichArticleResponse { article: article }))
}

#[delete("/<slug_>", format = "application/json")]
fn delete(connection: DbConnection, current_user: CurrentUser, slug_: String) -> ApiResult<()> {
    let current_user = current_user?;
    let article = Article::load_by_slug(&slug_, &*connection)?;
    if article.author_id != current_user.id {
        return Err(ApiError::Forbidden);
    }

    diesel_delete(&article).execute(&*connection)?;
    Ok(Json(()))
}

#[derive(FromForm, Default, Debug)]
struct ListFilter {
    tag: Option<String>,
    author: Option<String>,
    favorited: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ListResponse<'a> {
    articles: Vec<RichArticle<'a>>,
    articles_count: usize,
}

#[get("/?<filter>", format = "application/json")]
fn list<'a>(
    conn: DbConnection,
    current_user: CurrentUser,
    filter: ListFilter,
) -> ApiResult<ListResponse<'a>> {
    handle_list(conn, current_user, filter)
}

#[get("/", format = "application/json")]
fn list_without_params<'a>(
    conn: DbConnection,
    current_user: CurrentUser,
) -> ApiResult<ListResponse<'a>> {
    handle_list(conn, current_user, ListFilter::default())
}

fn handle_list<'a>(
    conn: DbConnection,
    current_user: CurrentUser,
    articles_filter: ListFilter,
) -> ApiResult<ListResponse<'a>> {
    use db::schema::*;

    let mut query = articles::table
        .inner_join(users::table.on(articles::author_id.eq(users::id)))
        .into_boxed::<Pg>();
    if let Some(author) = articles_filter.author {
        query = query.filter(users::username.eq(author));
    }

    if let Some(favorited_by) = articles_filter.favorited {
        let fav_articles = favorites::table
            .select(favorites::article_id)
            .filter(users::username.eq(favorited_by));
        query = query.filter(articles::id.eq_any(fav_articles));
    }

    if let Some(tag) = articles_filter.tag {
        let cond = articles::tag_list.contains(vec![tag]);
        query = query.filter(cond);
    }

    if let Some(offset) = articles_filter.offset {
        query = query.offset(offset);
    }

    let limit = articles_filter.limit.unwrap_or(20);

    query = query.limit(limit);

    let articles = query.get_results::<(Article, User)>(&*conn)?;
    let article_ids = articles.iter().map(|elem| elem.0.id).collect::<Vec<i32>>();

    let mut fav_count = favorites::table
        .select(sql::<(Integer, BigInt)>("article_id, count(user_id)"))
        .group_by(favorites::article_id)
        .filter(favorites::article_id.eq(any(article_ids.clone())))
        .get_results::<(i32, i64)>(&*conn)?
        .into_iter()
        .collect::<HashMap<_, _>>();
    match current_user {
        Ok(user) => {
            let authors = articles.iter().map(|elem| elem.1.id).collect::<Vec<i32>>();
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

            let favorited = exists(
                favorites::table.select(sql::<Integer>("1")).filter(
                    favorites::user_id
                        .eq(user.id)
                        .and(articles::id.eq(favorites::article_id)),
                ),
            );

            let mut favorited = articles::table
                .select((articles::id, favorited))
                .filter(articles::id.eq(any(article_ids.clone())))
                .get_results::<(i32, bool)>(&*conn)?
                .into_iter()
                .collect::<HashMap<_, _>>();

            let rich_articles = articles
                .into_iter()
                .map(|elem| {
                    let article = elem.0;
                    let user = elem.1;
                    let follows_user = follows.remove(&user.id).unwrap_or(false);
                    let favorites_count = fav_count.remove(&article.id).unwrap_or(0);
                    let favorited_by_user = favorited.remove(&article.id).unwrap_or(false);
                    let profile = user.profile(follows_user);
                    RichArticle::from(article, profile, Some(favorites_count), favorited_by_user)
                })
                .collect::<Vec<RichArticle>>();
            let count = rich_articles.len();
            Ok(Json(ListResponse {
                articles: rich_articles,
                articles_count: count,
            }))
        }
        Err(_) => {
            let rich_articles = articles
                .into_iter()
                .map(|elem| {
                    let article = elem.0;
                    let user = elem.1;
                    let favorites_count = fav_count.remove(&article.id).unwrap_or(0);
                    let profile = user.profile(false);
                    RichArticle::from(article, profile, Some(favorites_count), false)
                })
                .collect::<Vec<RichArticle>>();
            let count = rich_articles.len();
            Ok(Json(ListResponse {
                articles: rich_articles,
                articles_count: count,
            }))
        }
    }
}

#[derive(Debug, Serialize)]
struct TagList {
    tags: Vec<String>,
}

sql_function!(unnest, unnest_t, (a: Array<Text>) -> Text);

#[get("/tags", format = "application/json")]
fn tags(conn: DbConnection) -> ApiResult<TagList> {
    use db::schema::articles::dsl::*;
    let tags = articles
        .select(unnest(tag_list))
        .distinct()
        .get_results::<String>(&*conn)?;
    Ok(Json(TagList { tags }))
}

#[derive(Debug, FromForm, Default)]
struct Pagination {
    limit: Option<i64>,
    offset: Option<i64>,
}

#[get("/feed?<pagination>", format = "application/json")]
fn feed(
    conn: DbConnection,
    current_user: Result<User, ApiError>,
    pagination: Pagination,
) -> ApiResult<ListResponse<'static>> {
    handle_feed(current_user?, conn, pagination)
}

#[get("/feed", format = "application/json")]
fn feed_without_params(
    conn: DbConnection,
    current_user: Result<User, ApiError>,
) -> ApiResult<ListResponse<'static>> {
    handle_feed(current_user?, conn, Pagination::default())
}

fn handle_feed(
    current_user: User,
    conn: DbConnection,
    pagination: Pagination,
) -> ApiResult<ListResponse<'static>> {
    let following = followers::table
        .select(followers::user_id)
        .filter(followers::follower_id.eq(&current_user.id))
        .get_results::<i32>(&*conn)?;

    let query = articles::table
        .inner_join(users::table.on(articles::author_id.eq(users::id)))
        .filter(articles::author_id.eq_any(following))
        .offset(pagination.offset.unwrap_or(0))
        .limit(pagination.limit.unwrap_or(20));

    let data = query.get_results::<(Article, User)>(&*conn)?;
    let article_ids = data.iter().map(|elem| elem.0.id).collect::<Vec<i32>>();

    let mut fav_count = favorites::table
        .select(sql::<(Integer, BigInt)>("article_id, count(user_id)"))
        .group_by(favorites::article_id)
        .filter(favorites::article_id.eq(any(article_ids.clone())))
        .get_results::<(i32, i64)>(&*conn)?
        .into_iter()
        .collect::<HashMap<_, _>>();

    let favorited = exists(
        favorites::table.select(sql::<Integer>("1")).filter(
            favorites::user_id
                .eq(current_user.id)
                .and(articles::id.eq(favorites::article_id)),
        ),
    );

    let mut favorited = articles::table
        .select((articles::id, favorited))
        .filter(articles::id.eq(any(article_ids.clone())))
        .get_results::<(i32, bool)>(&*conn)?
        .into_iter()
        .collect::<HashMap<_, _>>();

    let rich_articles = data.into_iter()
        .map(|elem| {
            let article = elem.0;
            let user = elem.1;
            let favorites_count = fav_count.remove(&article.id).unwrap_or(0);
            let favorited_by_user = favorited.remove(&article.id).unwrap_or(false);
            let profile = user.profile(true);
            RichArticle::from(article, profile, Some(favorites_count), favorited_by_user)
        })
        .collect::<Vec<RichArticle>>();
    let count = rich_articles.len();
    Ok(Json(ListResponse {
        articles: rich_articles,
        articles_count: count,
    }))
}
