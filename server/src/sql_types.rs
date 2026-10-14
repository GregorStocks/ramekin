//! SQL types for columns SQLite has no native type for. Each is stored as
//! TEXT; these let the schema name what the text holds so Diesel maps it to
//! the right Rust type (see `crate::ids`).

/// A UUID stored as its hyphenated lowercase text form.
#[derive(diesel::sql_types::SqlType, diesel::query_builder::QueryId, Clone, Copy, Debug)]
#[diesel(sqlite_type(name = "Text"))]
pub struct UuidText;

/// A list of UUIDs stored as a JSON array of strings (`recipe_versions.photo_ids`).
#[derive(diesel::sql_types::SqlType, diesel::query_builder::QueryId, Clone, Copy, Debug)]
#[diesel(sqlite_type(name = "Text"))]
pub struct UuidList;
