use users::models::User;
use users::CurrentUser;
use types::*;
use rocket_contrib::Json;
use db::DbConnection;
use diesel::prelude::*;
use diesel::{debug_query, select};
use diesel::result::{DatabaseErrorKind, Error};
use db::schema::{articles, users};
use chrono::{Local, NaiveDateTime, Utc};
use regex::Regex;
use slug::slugify;
use diesel::{insert_into, sql_query, update as diesel_update};
use profile::Profile;
use diesel::dsl::{count, exists, Eq, Filter};
use diesel::sql_types::{BigInt, Bool, Integer, Nullable, Text, Timestamp};
use diesel::pg::types::sql_types::Array;
use serde::ser::{Serialize, SerializeStruct, Serializer};
use std::borrow::Cow;
use chrono::format::{Fixed, Item, Numeric, Pad};
use diesel::expression::{AsExpression, Expression};

static SELECT_RICH_ARTICLE: &str = "select articles.id as id,
       articles.slug as slug,
       articles.title as title,
       articles.description as description,
       articles.body as body,
       articles.tag_list as tag_list,
       articles.created_at as created_at,
       articles.updated_at as updated_at,
       coalesce(favorites_count, 0) as favorites_count,
       is_favorited as favorited,
       users.bio as bio,
       users.image as image,
       users.username as username,
       following
  from articles  LEFT JOIN (select count(favorites.article_id) as favorites_count, favorites.article_id  from favorites GROUP BY favorites.article_id) as favorited_count on articles.id = favorited_count.article_id
                      LEFT JOIN (select article_id, BOOL(article_id) as is_favorited from favorites where favorites.user_id = $1) as userfavorites on articles.id = userfavorites.article_id
                      LEFT JOIN (select follower_id, BOOL(follower_id) as following from followers where followers.user_id = $2) as userfollowers on articles.author_id = userfollowers.follower_id
                      INNER JOIN users on users.id = articles.author_id
                      where articles.slug = $3;";

static SELECT_RICH_ARTICLE_UNAUTHORIZED: &str = "select articles.id as id,
       articles.slug as slug,
       articles.title as title,
       articles.description as description,
       articles.body as body,
       articles.tag_list as tag_list,
       articles.created_at as created_at,
       articles.updated_at as updated_at,
       CAST(0 AS BIGINT) as favorites_count,
       false as favorited,
       users.bio as bio,
       users.image as image,
       users.username as username,
       false as following
  from articles  LEFT JOIN (select count(favorites.article_id) as favorites_count, favorites.article_id  from favorites GROUP BY favorites.article_id) as favorited_count on articles.id = favorited_count.article_id
                      INNER JOIN users on users.id = articles.author_id
                      where articles.slug = $1;";

#[derive(Identifiable, Queryable, Associations, PartialEq, Debug, Deserialize, Serialize,
         AsChangeset)]
#[belongs_to(User, foreign_key = "articles_users_id_fk")]
#[table_name = "articles"]
#[serde(rename_all = "camelCase")]
pub struct Article {
    #[serde(skip_serializing)]
    id: i32,

    #[serde(skip_serializing)]
    author_id: i32,
    slug: String,
    title: String,
    description: String,
    body: String,
    tag_list: Option<Vec<String>>,
    created_at: NaiveDateTime,
    updated_at: Option<NaiveDateTime>,
}

#[derive(Debug, Serialize)]
pub struct RichArticleResponse<'r> {
    article: RichArticle<'r>,
}

#[derive(Debug, QueryableByName, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RichArticle<'a> {
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
    #[sql_type = "Timestamp"]
    #[column_name = "created_at"]
    created_at: NaiveDateTime,
    #[sql_type = "Nullable<Timestamp>"]
    #[column_name = "updated_at"]
    updated_at: Option<NaiveDateTime>,
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
            tag_list: article.tag_list,
            created_at: article.created_at,
            updated_at: article.updated_at,
            favorites_count: favorites_count,
            favorited: favorited,
            author: author,
        }
    }

    fn by_slug<'r>(article_slug: &'r str) -> BySlug<'r> {
        use db::schema::articles::dsl::*;
        let condition = slug.eq(article_slug);
        articles.filter(condition)
    }
}

impl Article {
    pub fn load_by_slug(slug_: &str, connection: &PgConnection) -> Result<Article, ApiError> {
        use db::schema::articles::dsl::*;
        articles
            .filter(slug.eq(&slug_))
            .get_result::<Article>(connection)
            .map_err(|e| e.into())
    }

    fn by_slug<'r>(article_slug: &'r str) -> BySlug<'r> {
        use db::schema::articles::dsl::*;
        let condition = slug.eq(article_slug);
        articles.filter(condition)
    }
}

#[derive(Deserialize, Insertable, Serialize)]
#[allow(non_snake_case)]
#[table_name = "articles"]
pub struct NewArticle {
    author_id: i32,
    slug: String,
    title: String,
    description: String,
    body: String,
    tag_list: Option<Vec<String>>,
    created_at: NaiveDateTime,
    updated_at: Option<NaiveDateTime>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
pub struct ArticleDetails {
    title: String,
    description: String,
    body: String,
    #[serde(default)]
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
        created_at: created.naive_utc(),
        updated_at: Some(created.naive_utc()),
        tag_list: Some(create.article.tag_list),
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

    article.updated_at = Some(Utc::now().naive_utc());

    diesel_update(&article).set(&article).execute(&*connection)?;
    let favorited_count = favorites
        .select(count(user_id))
        .filter(article_id.eq(&article.id))
        .first(&*connection)?;

    let favorited = select(exists(
        favorites
            .filter(article_id.eq(&article.id))
            .filter(user_id.eq(&current_user.id)),
    )).get_result::<bool>(&*connection)?;

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
    use db::schema::favorites::dsl::*;
    let rich_article = match current_user {
        Ok(user) => sql_query(SELECT_RICH_ARTICLE)
            .bind::<Integer, _>(user.id)
            .bind::<Integer, _>(user.id)
            .bind::<Text, _>(slug_)
            .get_result(&*connection),
        Err(e) => match e {
            ApiError::Internal => return Err(e),
            _ => sql_query(SELECT_RICH_ARTICLE_UNAUTHORIZED)
                .bind::<Text, _>(slug_)
                .get_result(&*connection),
        },
    };
    println!("{:?}", rich_article);
    Ok(Json(RichArticleResponse {
        article: rich_article?,
    }))
}

#[post("/<slug>/favorite", format = "application/json")]
pub fn favorite(
    slug: String,
    connection: DbConnection,
    current_user: CurrentUser,
) -> ApiResult<RichArticleResponse<'static>> {
    use db::schema::favorites::dsl::*;
    use db::schema::articles::dsl as articles_dsl;

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

    let article = sql_query(SELECT_RICH_ARTICLE)
        .bind::<Integer, _>(current_user.id)
        .bind::<Integer, _>(current_user.id)
        .bind::<Text, _>(slug)
        .get_result::<RichArticle>(&*connection)?;

    Ok(Json(RichArticleResponse { article: article }))
}
