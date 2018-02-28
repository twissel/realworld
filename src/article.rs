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
use diesel::{insert_into, sql_query};
use profile::Profile;
use diesel::dsl::count;
use diesel::sql_types::{BigInt, Bool, Integer, Nullable, Text, Timestamp};
use diesel::pg::types::sql_types::Array;
use serde::ser::{Serialize, SerializeStruct, Serializer};
use std::borrow::Cow;
use chrono::format::{Fixed, Item, Numeric, Pad};

static SELECT_REACH_ARTICLE: &str = "select articles.id as id,
       articles.slug as slug,
       articles.title as title,
       articles.description as description,
       articles.body as body,
       articles.\"tagList\" as \"tagList\",
       articles.\"createdAt\" as \"createdAt\",
       articles.\"updatedAt\" as \"updatedAt\",
       coalesce(favorites_count, 0) as favorites_count,
       is_favorited as favorited,
       users.bio as author_bio,
       users.image as author_image,
       users.username as author_name,
       followed
  from articles  LEFT JOIN (select count(favorites.article_id) as favorites_count, favorites.article_id  from favorites GROUP BY favorites.article_id) as favorited_count on articles.id = favorited_count.article_id
                      LEFT JOIN (select article_id, BOOL(article_id) as is_favorited from favorites where favorites.user_id = $1) as userfavorites on articles.id = userfavorites.article_id
                      LEFT JOIN (select follower_id, BOOL(follower_id) as followed from followers where followers.user_id = 2) as userfollowers on articles.author_id = userfollowers.follower_id
                      INNER JOIN users on users.id = articles.author_id
                      where articles.slug = $2;";

#[derive(Identifiable, Queryable, Associations, PartialEq, Debug, Deserialize, Serialize)]
#[belongs_to(User, foreign_key = "articles_users_id_fk")]
#[table_name = "articles"]
#[allow(non_snake_case)]
pub struct Article {
    #[serde(skip_serializing)]
    id: i32,

    #[serde(skip_serializing)]
    author_id: i32,
    slug: String,
    title: String,
    description: String,
    body: String,
    tagList: Option<Vec<String>>,
    createdAt: NaiveDateTime,
    updatedAt: Option<NaiveDateTime>,
}

#[derive(Debug, Serialize)]
pub struct ReachArticleResponse {
    article: ReachArticle,
}

#[derive(Debug, QueryableByName)]
pub struct ReachArticle {
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
    #[column_name = "\"tagList\""]
    tagList: Option<Vec<String>>,
    #[sql_type = "Timestamp"]
    #[column_name = "\"createdAt\""]
    createdAt: NaiveDateTime,
    #[sql_type = "Nullable<Timestamp>"]
    #[column_name = "\"updatedAt\""]
    updatedAt: Option<NaiveDateTime>,
    #[sql_type = "BigInt"]
    favorites_count: i64,

    #[sql_type = "Bool"]
    favorited: bool,
    #[sql_type = "Nullable<Text>"]
    author_bio: Option<String>,
    #[sql_type = "Nullable<Text>"]
    author_image: Option<String>,
    #[sql_type = "Text"]
    author_name: String,

    #[sql_type = "Bool"]
    followed: bool,
}

impl ReachArticle {
    fn from(
        article: Article,
        author: Profile,
        favorites_count: Option<i64>,
        favorited: bool,
    ) -> Self {
        let favorites_count = match favorites_count {
            Some(c) => c,
            None => 0,
        };
        ReachArticle {
            id: article.id,
            slug: article.slug,
            title: article.title,
            description: article.description,
            body: article.body,
            tagList: article.tagList,
            createdAt: article.createdAt,
            updatedAt: article.updatedAt,
            favorites_count: favorites_count,
            favorited: favorited,
            author_bio: author.bio.map(|s| s.into_owned()),
            author_name: author.username.into_owned(),
            author_image: author.image.map(|s| s.into_owned()),
            followed: author.following,
        }
    }
}

impl Serialize for ReachArticle {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let bio = &self.author_bio;
        let image = &self.author_image;
        let profile = Profile {
            username: Cow::Borrowed(&self.author_name),
            bio: bio.as_ref().map(|bio| Cow::Borrowed(bio.as_str())),
            following: self.followed,
            image: image.as_ref().map(|image| Cow::Borrowed(image.as_str())),
        };

        let mut s = serializer.serialize_struct("ReachArticle", 10)?;
        s.serialize_field("slug", &self.slug)?;
        s.serialize_field("title", &self.title)?;
        s.serialize_field("description", &self.description)?;
        s.serialize_field("author", &profile)?;
        s.serialize_field("favoritesCount", &self.favorites_count)?;
        s.serialize_field("favorited", &self.favorited)?;
        s.serialize_field("createdAt", &self.createdAt)?;
        match &self.updatedAt {
            &Some(_) => s.serialize_field("updatedAt", &self.updatedAt)?,
            &None => s.serialize_field("updatedAt", &self.createdAt)?,
        }

        s.serialize_field("body", &self.body)?;
        s.serialize_field("tagList", &self.tagList)?;
        s.end()
    }
}

impl Article {
    pub fn favorited(&self, user: User) {}
    pub fn load_by_slug(slug_: &str, connection: &PgConnection) -> Result<Article, ApiError> {
        use db::schema::articles::dsl::*;
        articles
            .filter(slug.eq(&slug_))
            .get_result::<Article>(connection)
            .map_err(|e| e.into())
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
    tagList: Option<Vec<String>>,
    createdAt: NaiveDateTime,
    updatedAt: Option<NaiveDateTime>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
pub struct ArticleDetails {
    title: String,
    description: String,
    body: String,
    #[serde(default)]
    tagList: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateArticle {
    article: ArticleDetails,
}

impl Validate for CreateArticle {
    type Error = ValidationError;
    fn validate(self, _connection: &PgConnection) -> Result<Self, ValidationError> {
        let mut error = ValidationError::default();
        if self.article.body.trim().len() == 0 {
            error.add_error("body", "empty body");
        }

        if self.article.title.trim().len() == 0 {
            error.add_error("title", "empty title");
        }

        if self.article.description.trim().len() == 0 {
            error.add_error("description", "empty description");
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
) -> ApiResult<ReachArticleResponse> {
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
        createdAt: created.naive_utc(),
        updatedAt: None,
        tagList: Some(create.article.tagList),
    };
    let article = insert_into(articles)
        .values(&new_article)
        .get_result::<Article>(&*connection)?;
    let author = Profile {
        bio: user.bio.as_ref().map(|s| Cow::Borrowed(s.as_ref())),
        username: Cow::Borrowed(&*user.username),
        image: user.image.as_ref().map(|s| Cow::Borrowed(s.as_ref())),
        following: false,
    };
    let reach_article = ReachArticle::from(article, author, None, false);
    Ok(Json(ReachArticleResponse {
        article: reach_article,
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
) -> ApiResult<()> {
    let current_user = current_user?;
    Ok(Json(()))
}

#[get("/<slug>", format = "application/json")]
pub fn get(
    slug: String,
    connection: DbConnection,
    current_user: CurrentUser,
) -> ApiResult<ReachArticleResponse> {
    let current_user = current_user?;

    let article = sql_query(SELECT_REACH_ARTICLE)
        .bind::<Integer, _>(current_user.id)
        .bind::<Text, _>(slug)
        .get_result::<ReachArticle>(&*connection)?;

    Ok(Json(ReachArticleResponse { article: article }))
}

#[post("/<slug>/favorite", format = "application/json")]
pub fn favorite(
    slug: String,
    connection: DbConnection,
    current_user: CurrentUser,
) -> ApiResult<ReachArticleResponse> {
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

    let article = sql_query(SELECT_REACH_ARTICLE)
        .bind::<Integer, _>(current_user.id)
        .bind::<Text, _>(slug)
        .get_result::<ReachArticle>(&*connection)?;

    Ok(Json(ReachArticleResponse { article: article }))
}
