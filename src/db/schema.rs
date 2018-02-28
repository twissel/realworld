table! {
    articles (id) {
        id -> Int4,
        author_id -> Int4,
        slug -> Text,
        title -> Text,
        description -> Text,
        body -> Text,
        tagList -> Nullable<Array<Text>>,
        createdAt -> Timestamp,
        updatedAt -> Nullable<Timestamp>,
    }
}

table! {
    favorites (id) {
        id -> Int4,
        article_id -> Int4,
        user_id -> Int4,
    }
}

table! {
    followers (id) {
        id -> Int4,
        user_id -> Int4,
        follower_id -> Int4,
    }
}

table! {
    users (id) {
        id -> Int4,
        username -> Varchar,
        token -> Text,
        email -> Text,
        bio -> Nullable<Text>,
        image -> Nullable<Text>,
    }
}
