// @generated automatically by Diesel CLI.

diesel::table! {
    meal_plans (id) {
        id -> Uuid,
        user_id -> Uuid,
        recipe_id -> Uuid,
        meal_date -> Date,
        #[max_length = 20]
        meal_type -> Varchar,
        notes -> Nullable<Text>,
        created_at -> Timestamptz,
        deleted_at -> Nullable<Timestamptz>,
    }
}

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
    recipe_version_tags (recipe_version_id, tag_id) {
        recipe_version_id -> Uuid,
        tag_id -> Uuid,
    }
}

diesel::table! {
    recipe_versions (id) {
        id -> Uuid,
        recipe_id -> Uuid,
        title -> Varchar,
        description -> Nullable<Text>,
        ingredients -> Jsonb,
        instructions -> Text,
        source_url -> Nullable<Varchar>,
        source_name -> Nullable<Varchar>,
        photo_ids -> Array<Nullable<Uuid>>,
        servings -> Nullable<Text>,
        prep_time -> Nullable<Text>,
        cook_time -> Nullable<Text>,
        total_time -> Nullable<Text>,
        rating -> Nullable<Int4>,
        difficulty -> Nullable<Text>,
        nutritional_info -> Nullable<Text>,
        notes -> Nullable<Text>,
        version_source -> Varchar,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    recipes (id) {
        id -> Uuid,
        user_id -> Uuid,
        current_version_id -> Nullable<Uuid>,
        created_at -> Timestamptz,
        deleted_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    scrape_jobs (id) {
        id -> Uuid,
        user_id -> Uuid,
        url -> Nullable<Varchar>,
        status -> Varchar,
        failed_at_step -> Nullable<Varchar>,
        recipe_id -> Nullable<Uuid>,
        error_message -> Nullable<Text>,
        retry_count -> Int4,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        current_step -> Nullable<Varchar>,
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
    shopping_list_items (id) {
        id -> Uuid,
        user_id -> Uuid,
        item -> Text,
        amount -> Nullable<Text>,
        note -> Nullable<Text>,
        source_recipe_id -> Nullable<Uuid>,
        source_recipe_title -> Nullable<Text>,
        is_checked -> Bool,
        sort_order -> Int4,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        client_id -> Nullable<Uuid>,
        version -> Int4,
    }
}

diesel::table! {
    step_outputs (id) {
        id -> Uuid,
        scrape_job_id -> Uuid,
        step_name -> Varchar,
        build_id -> Varchar,
        output -> Jsonb,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    user_tags (id) {
        id -> Uuid,
        user_id -> Uuid,
        name -> Citext,
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

diesel::joinable!(meal_plans -> recipes (recipe_id));
diesel::joinable!(meal_plans -> users (user_id));
diesel::joinable!(photos -> users (user_id));
diesel::joinable!(recipe_version_tags -> recipe_versions (recipe_version_id));
diesel::joinable!(recipe_version_tags -> user_tags (tag_id));
diesel::joinable!(recipes -> users (user_id));
diesel::joinable!(scrape_jobs -> users (user_id));
diesel::joinable!(sessions -> users (user_id));
diesel::joinable!(shopping_list_items -> recipes (source_recipe_id));
diesel::joinable!(shopping_list_items -> users (user_id));
diesel::joinable!(step_outputs -> scrape_jobs (scrape_job_id));
diesel::joinable!(user_tags -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    meal_plans,
    photos,
    recipe_version_tags,
    recipe_versions,
    recipes,
    scrape_jobs,
    sessions,
    shopping_list_items,
    step_outputs,
    user_tags,
    users,
);
