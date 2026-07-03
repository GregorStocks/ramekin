//! Shared write path for recipe versions.
//!
//! Every mutation that creates a new `recipe_versions` row and repoints
//! `recipes.current_version_id` must go through this module so the
//! transaction shape and tag carry-forward stay identical everywhere.

use crate::models::{NewRecipe, NewRecipeVersion, RecipeVersionTag};
use crate::schema::{recipe_version_tags, recipe_versions, recipes};
use crate::tags::upsert_user_tag;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use uuid::Uuid;

/// How a new recipe version gets its `recipe_version_tags` rows.
pub enum TagSource<'a> {
    /// No tags on the new version.
    None,
    /// Copy tag links from an existing version.
    CopyFrom(Uuid),
    /// Upsert `user_tags` by name and link them.
    Names { user_id: Uuid, names: &'a [String] },
    /// Copy from an existing version, then upsert and link additional names.
    CopyAndNames {
        from_version: Uuid,
        user_id: Uuid,
        names: &'a [String],
    },
}

#[derive(Debug)]
pub enum VersionWriteError {
    /// The compare-and-swap repoint matched no row: the recipe's current
    /// version changed (or the recipe was soft-deleted) since it was read.
    Stale,
    Db(diesel::result::Error),
}

impl From<diesel::result::Error> for VersionWriteError {
    fn from(value: diesel::result::Error) -> Self {
        Self::Db(value)
    }
}

/// Insert a bare `recipes` row and return its id. Call inside a transaction.
pub fn insert_recipe(conn: &mut PgConnection, user_id: Uuid) -> QueryResult<Uuid> {
    diesel::insert_into(recipes::table)
        .values(NewRecipe { user_id })
        .returning(recipes::id)
        .get_result(conn)
}

/// Insert a new version row, unconditionally repoint
/// `recipes.current_version_id`, and apply tags. Call inside a transaction.
/// Returns the new version id.
pub fn create_new_version(
    conn: &mut PgConnection,
    new_version: &NewRecipeVersion<'_>,
    tags: TagSource<'_>,
) -> QueryResult<Uuid> {
    let version_id = insert_version_row(conn, new_version)?;

    diesel::update(recipes::table.find(new_version.recipe_id))
        .set(recipes::current_version_id.eq(version_id))
        .execute(conn)?;

    apply_tags(conn, version_id, tags)?;
    Ok(version_id)
}

/// Like [`create_new_version`], but repoints only if `current_version_id`
/// still equals `expected_current` and the recipe is not soft-deleted;
/// otherwise returns [`VersionWriteError::Stale`], which rolls back the
/// caller's transaction.
pub fn create_new_version_cas(
    conn: &mut PgConnection,
    new_version: &NewRecipeVersion<'_>,
    tags: TagSource<'_>,
    expected_current: Option<Uuid>,
) -> Result<Uuid, VersionWriteError> {
    let version_id = insert_version_row(conn, new_version)?;

    let rows_updated = diesel::update(
        recipes::table
            .filter(recipes::id.eq(new_version.recipe_id))
            .filter(recipes::current_version_id.eq(expected_current))
            .filter(recipes::deleted_at.is_null()),
    )
    .set(recipes::current_version_id.eq(version_id))
    .execute(conn)?;

    if rows_updated == 0 {
        return Err(VersionWriteError::Stale);
    }

    apply_tags(conn, version_id, tags)?;
    Ok(version_id)
}

fn insert_version_row(
    conn: &mut PgConnection,
    new_version: &NewRecipeVersion<'_>,
) -> QueryResult<Uuid> {
    diesel::insert_into(recipe_versions::table)
        .values(new_version)
        .returning(recipe_versions::id)
        .get_result(conn)
}

fn apply_tags(conn: &mut PgConnection, version_id: Uuid, tags: TagSource<'_>) -> QueryResult<()> {
    match tags {
        TagSource::None => {}
        TagSource::CopyFrom(from_version) => {
            copy_recipe_version_tags(conn, from_version, version_id)?;
        }
        TagSource::Names { user_id, names } => {
            link_tags_by_name(conn, version_id, user_id, names)?;
        }
        TagSource::CopyAndNames {
            from_version,
            user_id,
            names,
        } => {
            copy_recipe_version_tags(conn, from_version, version_id)?;
            link_tags_by_name(conn, version_id, user_id, names)?;
        }
    }
    Ok(())
}

/// Copy every tag link from `old_version_id` onto `new_version_id`.
pub fn copy_recipe_version_tags(
    conn: &mut PgConnection,
    old_version_id: Uuid,
    new_version_id: Uuid,
) -> QueryResult<()> {
    let existing_tag_ids: Vec<Uuid> = recipe_version_tags::table
        .filter(recipe_version_tags::recipe_version_id.eq(old_version_id))
        .select(recipe_version_tags::tag_id)
        .load(conn)?;

    for tag_id in existing_tag_ids {
        link_tag(conn, new_version_id, tag_id)?;
    }

    Ok(())
}

fn link_tags_by_name(
    conn: &mut PgConnection,
    version_id: Uuid,
    user_id: Uuid,
    names: &[String],
) -> QueryResult<()> {
    for name in names {
        let tag_id = upsert_user_tag(conn, user_id, name)?;
        link_tag(conn, version_id, tag_id)?;
    }
    Ok(())
}

fn link_tag(conn: &mut PgConnection, version_id: Uuid, tag_id: Uuid) -> QueryResult<()> {
    diesel::insert_into(recipe_version_tags::table)
        .values(RecipeVersionTag {
            recipe_version_id: version_id,
            tag_id,
        })
        .on_conflict_do_nothing()
        .execute(conn)?;
    Ok(())
}
