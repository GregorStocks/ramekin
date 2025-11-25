// @generated automatically by Diesel CLI.

diesel::table! {
    photos (id) {
        id -> Uuid,
        user_id -> Uuid,
        content_type -> Varchar,
        data -> Nullable<Bytea>,
        url -> Nullable<Varchar>,
        created_at -> Timestamptz,
        deleted_at -> Nullable<Timestamptz>,
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
        tags -> Array<Nullable<Text>>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        deleted_at -> Nullable<Timestamptz>,
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
diesel::joinable!(sessions -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(photos, recipes, sessions, users,);
