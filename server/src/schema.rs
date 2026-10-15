// Hand-maintained: Diesel's SQLite schema printer can't tell UUID or JSON
// text from other TEXT columns. Keep in step with migrations/.

diesel::table! {
    use diesel::sql_types::*;
    use crate::sql_types::*;

    client_log_uploads (id) {
        id -> UuidText,
        user_id -> UuidText,
        platform -> Text,
        app_version -> Nullable<Text>,
        os_info -> Nullable<Text>,
        content -> Text,
        created_at -> TimestamptzSqlite,
        deleted_at -> Nullable<TimestamptzSqlite>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use crate::sql_types::*;

    data_migrations (name) {
        name -> Text,
        run_at -> TimestamptzSqlite,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use crate::sql_types::*;

    ingredient_name_resolutions (name) {
        name -> Text,
        status -> Text,
        disposition -> Nullable<Text>,
        catalog_key -> Nullable<Text>,
        model -> Nullable<Text>,
        error -> Nullable<Text>,
        attempts -> Integer,
        created_at -> TimestamptzSqlite,
        updated_at -> TimestamptzSqlite,
        kcal_per_100g -> Nullable<Double>,
        grams_per_cup -> Nullable<Double>,
        grams_per_piece -> Nullable<Double>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use crate::sql_types::*;

    meal_plans (id) {
        id -> UuidText,
        user_id -> UuidText,
        recipe_id -> UuidText,
        meal_date -> Date,
        meal_type -> Text,
        notes -> Nullable<Text>,
        created_at -> TimestamptzSqlite,
        deleted_at -> Nullable<TimestamptzSqlite>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use crate::sql_types::*;

    photo_thumbnails (id) {
        id -> UuidText,
        photo_id -> UuidText,
        size -> Integer,
        data -> Binary,
        created_at -> TimestamptzSqlite,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use crate::sql_types::*;

    photos (id) {
        id -> UuidText,
        user_id -> UuidText,
        content_type -> Text,
        data -> Binary,
        created_at -> TimestamptzSqlite,
        deleted_at -> Nullable<TimestamptzSqlite>,
        thumbnail -> Binary,
        width -> Nullable<Integer>,
        height -> Nullable<Integer>,
        file_size -> Nullable<Integer>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use crate::sql_types::*;

    recipe_version_tags (recipe_version_id, tag_id) {
        recipe_version_id -> UuidText,
        tag_id -> UuidText,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use crate::sql_types::*;

    recipe_versions (id) {
        id -> UuidText,
        recipe_id -> UuidText,
        title -> Text,
        description -> Nullable<Text>,
        ingredients -> Json,
        instructions -> Text,
        source_url -> Nullable<Text>,
        source_name -> Nullable<Text>,
        photo_ids -> UuidList,
        servings -> Nullable<Text>,
        prep_time -> Nullable<Text>,
        cook_time -> Nullable<Text>,
        total_time -> Nullable<Text>,
        rating -> Nullable<Integer>,
        difficulty -> Nullable<Text>,
        nutritional_info -> Nullable<Text>,
        notes -> Nullable<Text>,
        version_source -> Text,
        created_at -> TimestamptzSqlite,
        change_seq -> BigInt,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use crate::sql_types::*;

    recipes (id) {
        id -> UuidText,
        user_id -> UuidText,
        current_version_id -> Nullable<UuidText>,
        created_at -> TimestamptzSqlite,
        deleted_at -> Nullable<TimestamptzSqlite>,
        deleted_seq -> Nullable<BigInt>,
        seq -> BigInt,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use crate::sql_types::*;

    scrape_jobs (id) {
        id -> UuidText,
        user_id -> UuidText,
        url -> Nullable<Text>,
        status -> Text,
        failed_at_step -> Nullable<Text>,
        recipe_id -> Nullable<UuidText>,
        error_message -> Nullable<Text>,
        retry_count -> Integer,
        created_at -> TimestamptzSqlite,
        updated_at -> TimestamptzSqlite,
        current_step -> Nullable<Text>,
        photo_only -> Bool,
        current_step_started_at -> Nullable<TimestamptzSqlite>,
        expected_version_id -> Nullable<UuidText>,
        idempotency_key -> Nullable<Text>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use crate::sql_types::*;

    sessions (id) {
        id -> UuidText,
        user_id -> UuidText,
        token_hash -> Text,
        expires_at -> TimestamptzSqlite,
        created_at -> TimestamptzSqlite,
        token_type -> Text,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use crate::sql_types::*;

    shopping_list_items (id) {
        id -> UuidText,
        user_id -> UuidText,
        item -> Text,
        amount -> Nullable<Text>,
        note -> Nullable<Text>,
        source_recipe_id -> Nullable<UuidText>,
        source_recipe_title -> Nullable<Text>,
        is_checked -> Bool,
        sort_order -> Integer,
        created_at -> TimestamptzSqlite,
        updated_at -> TimestamptzSqlite,
        client_id -> Nullable<UuidText>,
        version -> Integer,
        deleted_at -> Nullable<TimestamptzSqlite>,
        category_override -> Nullable<Text>,
        change_seq -> BigInt,
        computed_category -> Nullable<Text>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use crate::sql_types::*;

    step_outputs (id) {
        id -> UuidText,
        scrape_job_id -> UuidText,
        step_name -> Text,
        build_id -> Text,
        output -> CompressedJson,
        created_at -> TimestamptzSqlite,
        duration_ms -> Nullable<BigInt>,
        summary -> Nullable<Text>,
        success -> Bool,
        error -> Nullable<Text>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use crate::sql_types::*;

    user_tags (id) {
        id -> UuidText,
        user_id -> UuidText,
        name -> Text,
        created_at -> TimestamptzSqlite,
        deleted_at -> Nullable<TimestamptzSqlite>,
        updated_at -> TimestamptzSqlite,
        change_seq -> BigInt,
        name_key -> Text,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use crate::sql_types::*;

    users (id) {
        id -> UuidText,
        username -> Text,
        password_hash -> Text,
        created_at -> TimestamptzSqlite,
        updated_at -> TimestamptzSqlite,
        deleted_at -> Nullable<TimestamptzSqlite>,
        username_key -> Text,
    }
}

diesel::table! {
    sync_clock (id) {
        id -> BigInt,
        value -> BigInt,
    }
}

diesel::joinable!(client_log_uploads -> users (user_id));
diesel::joinable!(meal_plans -> recipes (recipe_id));
diesel::joinable!(meal_plans -> users (user_id));
diesel::joinable!(photo_thumbnails -> photos (photo_id));
diesel::joinable!(photos -> users (user_id));
diesel::joinable!(recipe_version_tags -> recipe_versions (recipe_version_id));
diesel::joinable!(recipe_version_tags -> user_tags (tag_id));
diesel::joinable!(recipes -> users (user_id));
diesel::joinable!(scrape_jobs -> recipe_versions (expected_version_id));
diesel::joinable!(scrape_jobs -> users (user_id));
diesel::joinable!(sessions -> users (user_id));
diesel::joinable!(shopping_list_items -> recipes (source_recipe_id));
diesel::joinable!(shopping_list_items -> users (user_id));
diesel::joinable!(step_outputs -> scrape_jobs (scrape_job_id));
diesel::joinable!(user_tags -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    client_log_uploads,
    data_migrations,
    ingredient_name_resolutions,
    meal_plans,
    photo_thumbnails,
    photos,
    recipe_version_tags,
    recipe_versions,
    recipes,
    scrape_jobs,
    sessions,
    shopping_list_items,
    step_outputs,
    sync_clock,
    user_tags,
    users,
);
