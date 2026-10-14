//! Id types the database stores as text.
//!
//! Diesel's SQLite backend has no UUID type, and Rust's orphan rules keep
//! this crate from teaching Diesel to bind `uuid::Uuid` itself, so the
//! server uses this local wrapper everywhere instead. It serializes, parses
//! and documents (OpenAPI `format: uuid`) exactly like `uuid::Uuid`.

use crate::sql_types::{UuidList, UuidText};
use diesel::deserialize::{self, FromSql, FromSqlRow};
use diesel::expression::AsExpression;
use diesel::serialize::{self, IsNull, Output, ToSql};
use diesel::sql_types::Text;
use diesel::sqlite::{Sqlite, SqliteValue};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::{Deref, DerefMut};
use std::str::FromStr;

#[derive(
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    AsExpression,
    FromSqlRow,
)]
#[serde(transparent)]
#[diesel(sql_type = UuidText)]
pub struct Uuid(pub uuid::Uuid);

impl Uuid {
    pub fn new_v4() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    /// A new time-ordered id: what every inserted row gets.
    pub fn now_v7() -> Self {
        Self(uuid::Uuid::now_v7())
    }

    pub const fn nil() -> Self {
        Self(uuid::Uuid::nil())
    }

    pub fn parse_str(s: &str) -> Result<Self, uuid::Error> {
        uuid::Uuid::parse_str(s).map(Self)
    }
}

impl fmt::Debug for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl fmt::Display for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl FromStr for Uuid {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse_str(s)
    }
}

impl From<uuid::Uuid> for Uuid {
    fn from(id: uuid::Uuid) -> Self {
        Self(id)
    }
}

impl From<Uuid> for uuid::Uuid {
    fn from(id: Uuid) -> Self {
        id.0
    }
}

impl utoipa::PartialSchema for Uuid {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
        use utoipa::openapi::schema::{KnownFormat, ObjectBuilder, SchemaFormat, Type};
        ObjectBuilder::new()
            .schema_type(Type::String)
            .format(Some(SchemaFormat::KnownFormat(KnownFormat::Uuid)))
            .into()
    }
}

impl utoipa::ToSchema for Uuid {}

impl FromSql<UuidText, Sqlite> for Uuid {
    fn from_sql(value: SqliteValue<'_, '_, '_>) -> deserialize::Result<Self> {
        let text = <String as FromSql<Text, Sqlite>>::from_sql(value)?;
        Ok(Self::parse_str(&text)?)
    }
}

impl ToSql<UuidText, Sqlite> for Uuid {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Sqlite>) -> serialize::Result {
        out.set_value(self.0.hyphenated().to_string());
        Ok(IsNull::No)
    }
}

/// A recipe version's photos, in display order, stored as a JSON array.
#[derive(Clone, Debug, Default, PartialEq, Eq, AsExpression, FromSqlRow)]
#[diesel(sql_type = UuidList)]
pub struct PhotoIds(pub Vec<Option<Uuid>>);

impl Deref for PhotoIds {
    type Target = Vec<Option<Uuid>>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for PhotoIds {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<Vec<Option<Uuid>>> for PhotoIds {
    fn from(ids: Vec<Option<Uuid>>) -> Self {
        Self(ids)
    }
}

impl FromIterator<Option<Uuid>> for PhotoIds {
    fn from_iter<I: IntoIterator<Item = Option<Uuid>>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

impl IntoIterator for PhotoIds {
    type Item = Option<Uuid>;
    type IntoIter = std::vec::IntoIter<Option<Uuid>>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl FromSql<UuidList, Sqlite> for PhotoIds {
    fn from_sql(value: SqliteValue<'_, '_, '_>) -> deserialize::Result<Self> {
        let text = <String as FromSql<Text, Sqlite>>::from_sql(value)?;
        Ok(Self(serde_json::from_str(&text)?))
    }
}

impl ToSql<UuidList, Sqlite> for PhotoIds {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Sqlite>) -> serialize::Result {
        out.set_value(serde_json::to_string(&self.0)?);
        Ok(IsNull::No)
    }
}
