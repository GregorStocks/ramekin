//! What a scrape job does, stored in `scrape_jobs.kind` when the job is
//! created. Everything that used to infer it from which nullable columns
//! were set reads it from here instead.

use diesel::deserialize::{self, FromSql, FromSqlRow};
use diesel::expression::AsExpression;
use diesel::serialize::{self, IsNull, Output, ToSql};
use diesel::sql_types::Text;
use diesel::sqlite::{Sqlite, SqliteValue};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, AsExpression, FromSqlRow)]
#[diesel(sql_type = Text)]
pub enum JobKind {
    /// A new recipe from a URL the server fetches.
    UrlScrape,
    /// A new recipe from a page the bookmarklet captured: starts with the
    /// page's HTML already stored.
    Capture,
    /// A new recipe from a recipe the client already extracted (a Paprika
    /// import): starts at parse_ingredients.
    Import,
    /// A new recipe from pasted text, stored as the job's `source_text`.
    TextImport,
    /// A new recipe extracted from uploaded photos.
    PhotoImport,
    /// Refetch a recipe's page for its photos only (`photo_only`).
    PhotoRescrape,
    /// Refetch a recipe's page and save it as a new version. No longer
    /// created (normalize jobs with the refetch op replaced it); older rows
    /// keep it.
    Rescrape,
    /// Run normalize ops on a recipe, proposing a version.
    Normalize,
    /// Edit a recipe with AI, proposing a version.
    Edit,
}

impl JobKind {
    pub const ALL: [JobKind; 9] = [
        JobKind::UrlScrape,
        JobKind::Capture,
        JobKind::Import,
        JobKind::TextImport,
        JobKind::PhotoImport,
        JobKind::PhotoRescrape,
        JobKind::Rescrape,
        JobKind::Normalize,
        JobKind::Edit,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            JobKind::UrlScrape => "url_scrape",
            JobKind::Capture => "capture",
            JobKind::Import => "import",
            JobKind::TextImport => "text_import",
            JobKind::PhotoImport => "photo_import",
            JobKind::PhotoRescrape => "photo_rescrape",
            JobKind::Rescrape => "rescrape",
            JobKind::Normalize => "normalize",
            JobKind::Edit => "edit",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == s)
    }

    /// Whether the job proposes a version of an existing recipe for the user
    /// to accept, instead of saving one.
    pub const fn proposes(self) -> bool {
        matches!(self, JobKind::Normalize | JobKind::Edit)
    }

    /// Whether the job saves a new version of an existing recipe (its
    /// `recipe_id` and `expected_version_id` are set from the start).
    pub const fn updates_recipe(self) -> bool {
        matches!(self, JobKind::PhotoRescrape | JobKind::Rescrape)
    }
}

impl std::fmt::Display for JobKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromSql<Text, Sqlite> for JobKind {
    fn from_sql(value: SqliteValue<'_, '_, '_>) -> deserialize::Result<Self> {
        let text = <String as FromSql<Text, Sqlite>>::from_sql(value)?;
        Self::parse(&text).ok_or_else(|| format!("unknown scrape job kind {text:?}").into())
    }
}

impl ToSql<Text, Sqlite> for JobKind {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Sqlite>) -> serialize::Result {
        out.set_value(self.as_str());
        Ok(IsNull::No)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_round_trip_through_their_text() {
        for kind in JobKind::ALL {
            assert_eq!(JobKind::parse(kind.as_str()), Some(kind));
        }
        assert_eq!(JobKind::parse("scrape"), None);
    }

    #[test]
    fn only_normalize_and_edit_propose() {
        let proposing: Vec<_> = JobKind::ALL.into_iter().filter(|k| k.proposes()).collect();
        assert_eq!(proposing, vec![JobKind::Normalize, JobKind::Edit]);
    }
}
