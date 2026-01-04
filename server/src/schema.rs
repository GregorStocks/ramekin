// @generated automatically by Diesel CLI.

diesel::table! {
    photos (id) {
        id -> Uuid,
        user_id -> Uuid,
        content_type -> Varchar,
        data -> Bytea,
        created_at -> Timestamptz,
        deleted_at -> Nullable<Timestamptz>,
        thumbnail -> Bytea,
    }
}

diesel::table! {
    recipes (id) {
        id -> Uuid,
        user_id -> Uuid,
        title -> Varchar,
        description -> Nullable<Text>,
        ingredients -> Jsonb,
        instructions -> Text,
        source_url -> Nullable<Varchar>,
        source_name -> Nullable<Varchar>,
        photo_ids -> Array<Nullable<Uuid>>,
        tags -> Array<Nullable<Citext>>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        deleted_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    scrape_jobs (id) {
        id -> Uuid,
        user_id -> Uuid,
        url -> Varchar,
        status -> Varchar,
        failed_at_step -> Nullable<Varchar>,
        parsed_data -> Nullable<Jsonb>,
        recipe_id -> Nullable<Uuid>,
        error_message -> Nullable<Text>,
        retry_count -> Int4,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    sessions (id) {
        id -> Uuid,
        user_id -> Uuid,
        #[max_length = 255]
        token_hash -> Varchar,
        expires_at -> Timestamptz,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    url_cache (url) {
        url -> Varchar,
        content -> Bytea,
        content_type -> Nullable<Varchar>,
        fetched_at -> Timestamptz,
    }
}

diesel::table! {
    users (id) {
        id -> Uuid,
        #[max_length = 255]
        username -> Varchar,
        #[max_length = 255]
        password_hash -> Varchar,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        deleted_at -> Nullable<Timestamptz>,
    }
}

diesel::joinable!(photos -> users (user_id));
diesel::joinable!(recipes -> users (user_id));
diesel::joinable!(scrape_jobs -> recipes (recipe_id));
diesel::joinable!(scrape_jobs -> users (user_id));
diesel::joinable!(sessions -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    photos,
    recipes,
    scrape_jobs,
    sessions,
    url_cache,
    users,
);
