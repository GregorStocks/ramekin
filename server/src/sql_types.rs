//! SQL types for columns SQLite has no native type for. Most are stored as
//! TEXT; these let the schema name what the text holds so Diesel maps it to
//! the right Rust type (see `crate::ids`).

use diesel::deserialize::{self, FromSql};
use diesel::serialize::{self, IsNull, Output, ToSql};
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

/// JSON stored zstd-compressed in a `BLOB` (`step_outputs.output`, where a
/// fetched page runs to hundreds of KB). SQLite doesn't compress large values
/// the way Postgres's TOAST did. The column is declared `TEXT`, whose
/// affinity leaves blobs alone. Reads also accept plain JSON `TEXT`, which is
/// how rows written before compression (or by hand) hold it.
#[derive(diesel::sql_types::SqlType, diesel::query_builder::QueryId, Clone, Copy, Debug)]
#[diesel(sqlite_type(name = "Binary"))]
pub struct CompressedJson;

/// A JSON value to write to a `CompressedJson` column. (Reads load straight
/// into `serde_json::Value`; the orphan rules keep Diesel from writing one
/// without a local wrapper.)
#[derive(diesel::expression::AsExpression, Debug, Clone)]
#[diesel(sql_type = CompressedJson)]
pub struct Compressed(pub serde_json::Value);

/// zstd's default level: most of the size win at a fraction of the CPU of
/// the higher levels.
const ZSTD_LEVEL: i32 = 3;

/// Serialized JSON, zstd-compressed: the bytes a `CompressedJson` column
/// holds.
pub fn compress_json(json: &str) -> std::io::Result<Vec<u8>> {
    zstd::encode_all(json.as_bytes(), ZSTD_LEVEL)
}

impl FromSql<CompressedJson, Sqlite> for serde_json::Value {
    fn from_sql(mut value: SqliteValue<'_, '_, '_>) -> deserialize::Result<Self> {
        if value.value_type() == Some(diesel::sqlite::SqliteType::Text) {
            return Ok(serde_json::from_str(value.read_text())?);
        }
        let json = zstd::decode_all(value.read_blob())?;
        Ok(serde_json::from_slice(&json)?)
    }
}

impl ToSql<CompressedJson, Sqlite> for Compressed {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Sqlite>) -> serialize::Result {
        out.set_value(compress_json(&serde_json::to_string(&self.0)?)?);
        Ok(IsNull::No)
    }
}
