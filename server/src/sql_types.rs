//! SQL types for columns SQLite has no native type for. Each is stored as
//! TEXT; these let the schema name what the text holds so Diesel maps it to
//! the right Rust type (see `crate::ids`).

use diesel::deserialize::{self, FromSql};
use diesel::sql_types::Text;
use diesel::sqlite::{Sqlite, SqliteValue};

/// A UUID stored as its hyphenated lowercase text form.
#[derive(diesel::sql_types::SqlType, diesel::query_builder::QueryId, Clone, Copy, Debug)]
#[diesel(sqlite_type(name = "Text"))]
pub struct UuidText;

/// A list of UUIDs stored as a JSON array of strings (`recipe_versions.photo_ids`).
#[derive(diesel::sql_types::SqlType, diesel::query_builder::QueryId, Clone, Copy, Debug)]
#[diesel(sqlite_type(name = "Text"))]
pub struct UuidList;

/// A list of strings returned as a JSON array (`json_group_array`).
#[derive(diesel::sql_types::SqlType, diesel::query_builder::QueryId, Clone, Copy, Debug)]
#[diesel(sqlite_type(name = "Text"))]
pub struct TextList;

impl FromSql<TextList, Sqlite> for Vec<String> {
    fn from_sql(value: SqliteValue<'_, '_, '_>) -> deserialize::Result<Self> {
        let text = <String as FromSql<Text, Sqlite>>::from_sql(value)?;
        Ok(serde_json::from_str(&text)?)
    }
}
