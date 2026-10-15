use super::read::RecipeWithVersion;
use crate::ids::Uuid;
use crate::models::{Ingredient, Measurement};
use crate::photos::processing::{generate_thumbnail, resize_for_export, EXPORT_PHOTO_DATA_SIZE};
use crate::photos::store::live;
use crate::schema::{photos, recipe_version_tags, user_tags};
use base64::Engine;
use diesel::prelude::*;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::Write;

/// Paprika recipe format for export.
#[derive(Debug, Serialize)]
struct PaprikaRecipe {
    uid: String,
    name: String,
    ingredients: String,
    directions: String,
    description: String,
    notes: String,
    source: String,
    source_url: String,
    categories: Vec<String>,
    servings: String,
    prep_time: String,
    cook_time: String,
    total_time: String,
    rating: i32,
    difficulty: String,
    nutritional_info: String,
    created: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    photos: Vec<PaprikaPhoto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    photo_data: Option<String>,
    hash: String,
}

#[derive(Debug, Serialize)]
struct PaprikaPhoto {
    filename: String,
    hash: String,
    data: String,
}

/// Render stored ingredients as Paprika's newline-separated ingredients text.
///
/// Each line matches the clients' display format. A `Section:` header line
/// precedes each change of section, and a blank line ends a section when the
/// ingredients after it have none; the Paprika importer
/// (`parse_paprika_ingredients`) reads both back.
///
/// Two shapes keep their text but not their structure on re-import: an
/// alternative with only an amount or only a unit ("2 (pinch) eggs") comes back
/// as a note, and a note shaped like a measurement ("flour (2 tbsp)") comes back
/// as an alternative. The importer can't tell either from what Paprika users
/// type, where a trailing "(120 g)" is an alternative and "(3)" is a note.
///
/// Read-time derived grams are left out: they're approximate, and Paprika
/// would store them as source text.
fn format_ingredients_text(ingredients: &[Ingredient]) -> String {
    let mut lines = Vec::new();
    let mut current_section: Option<&str> = None;
    for ingredient in ingredients {
        let section = ingredient
            .section
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        if section != current_section {
            match section {
                Some(section) => lines.push(format!("{}:", section)),
                None => lines.push(String::new()),
            }
        }
        current_section = section;
        lines.push(format_ingredient_line(ingredient, true, true));
    }
    lines.join("\n")
}

/// Format one ingredient like the clients do
/// (shared-test-vectors/ingredient-formatting.json), without scaling or
/// derived grams: `primary (alt, alt) item (note)`.
fn format_ingredient_line(
    ingredient: &Ingredient,
    include_alternatives: bool,
    include_note: bool,
) -> String {
    let mut parts = Vec::new();
    let mut measurements = ingredient.measurements.iter();

    if let Some(primary) = measurements.next().and_then(format_measurement) {
        parts.push(primary);
    }

    if include_alternatives {
        let alternatives: Vec<String> = measurements.filter_map(format_measurement).collect();
        if !alternatives.is_empty() {
            parts.push(format!("({})", alternatives.join(", ")));
        }
    }

    parts.push(ingredient.item.clone());

    if include_note {
        if let Some(note) = ingredient.note.as_deref().and_then(non_blank) {
            parts.push(format!("({})", note));
        }
    }

    parts.join(" ")
}

fn format_measurement(measurement: &Measurement) -> Option<String> {
    let values: Vec<&str> = [measurement.amount.as_deref(), measurement.unit.as_deref()]
        .into_iter()
        .flatten()
        .filter_map(non_blank)
        .collect();
    (!values.is_empty()).then(|| values.join(" "))
}

fn non_blank(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then_some(trimmed)
}

/// Convert a Ramekin recipe to Paprika format.
fn convert_to_paprika(
    recipe: &RecipeWithVersion,
    photos_data: Vec<(Uuid, Vec<u8>)>,
    tags: Vec<String>,
) -> Result<PaprikaRecipe, String> {
    let version = &recipe.version;

    let ingredients: Vec<Ingredient> = serde_json::from_value(version.ingredients.clone())
        .map_err(|e| format!("stored ingredients JSON failed to deserialize: {}", e))?;
    let ingredients_str = format_ingredients_text(&ingredients);

    let fallback_photo_data = photos_data.first().and_then(|(id, raw)| {
        match generate_thumbnail(raw, EXPORT_PHOTO_DATA_SIZE) {
            Ok(thumbnail) => Some(base64::engine::general_purpose::STANDARD.encode(thumbnail)),
            Err(e) => {
                tracing::warn!(
                    photo_id = %id,
                    bytes = raw.len(),
                    error = %e,
                    "skipping export photo_data fallback; thumbnail generation failed"
                );
                None
            }
        }
    });

    // Each photo is downscaled before base64 encoding. Originals can be up to
    // MAX_FILE_SIZE (10MB) each and the export would otherwise hold every
    // photo's raw bytes plus a 1.33x base64 copy plus the JSON + gzip buffer
    // all live at once. Photos that fail to decode/resize are skipped with a
    // warning rather than failing the whole recipe export.
    let paprika_photos: Vec<PaprikaPhoto> = photos_data
        .into_iter()
        .filter_map(|(id, raw)| {
            let raw_len = raw.len();
            let resized = match resize_for_export(&raw) {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!(
                        photo_id = %id,
                        bytes = raw_len,
                        error = %e,
                        "skipping photo during export; resize failed"
                    );
                    return None;
                }
            };
            drop(raw);
            let filename = format!("{}.jpg", id);
            let mut hasher = Sha256::new();
            hasher.update(&resized);
            let hash = hex::encode_upper(hasher.finalize());
            let data_b64 = base64::engine::general_purpose::STANDARD.encode(&resized);
            Some(PaprikaPhoto {
                filename,
                hash,
                data: data_b64,
            })
        })
        .collect();

    // `photo_data` is only a fallback thumbnail when the structured `photos`
    // array is empty. Duplicating the first full photo here roughly doubles
    // the photo payload for no importer benefit.
    let photo_data = if paprika_photos.is_empty() {
        fallback_photo_data
    } else {
        None
    };

    let created = recipe.created_at.format("%Y-%m-%d %H:%M:%S").to_string();

    let recipe_content = format!(
        "{}{}{}{}",
        version.title,
        ingredients_str,
        version.instructions,
        version.description.as_deref().unwrap_or("")
    );
    let mut hasher = Sha256::new();
    hasher.update(recipe_content.as_bytes());
    let hash = hex::encode_upper(hasher.finalize());

    Ok(PaprikaRecipe {
        uid: recipe.id.to_string().to_uppercase(),
        name: version.title.clone(),
        ingredients: ingredients_str,
        directions: version.instructions.clone(),
        description: version.description.clone().unwrap_or_default(),
        notes: version.notes.clone().unwrap_or_default(),
        source: version.source_name.clone().unwrap_or_default(),
        source_url: version.source_url.clone().unwrap_or_default(),
        categories: tags,
        servings: version.servings.clone().unwrap_or_default(),
        prep_time: version.prep_time.clone().unwrap_or_default(),
        cook_time: version.cook_time.clone().unwrap_or_default(),
        total_time: version.total_time.clone().unwrap_or_default(),
        rating: version.rating.unwrap_or(0),
        difficulty: version.difficulty.clone().unwrap_or_default(),
        nutritional_info: version.nutritional_info.clone().unwrap_or_default(),
        created,
        photos: paprika_photos,
        photo_data,
        hash,
    })
}

/// Compress a recipe to gzip format (for .paprikarecipe files).
fn gzip_recipe(recipe: &PaprikaRecipe) -> Result<Vec<u8>, String> {
    let json = serde_json::to_string(recipe).map_err(|e| e.to_string())?;
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(json.as_bytes())
        .map_err(|e: std::io::Error| e.to_string())?;
    encoder.finish().map_err(|e: std::io::Error| e.to_string())
}

/// Fetch all photo data for a recipe.
fn fetch_recipe_photos(
    conn: &mut diesel::SqliteConnection,
    user_id: Uuid,
    photo_ids: &[Option<Uuid>],
) -> Result<Vec<(Uuid, Vec<u8>)>, String> {
    let ids: Vec<Uuid> = photo_ids.iter().filter_map(|id| *id).collect();
    if ids.is_empty() {
        return Ok(Vec::new());
    }

    let hashes = photos::table
        .filter(photos::id.eq_any(&ids))
        .filter(photos::user_id.eq(user_id))
        .filter(photos::deleted_at.is_null())
        .select((photos::id, photos::data_hash))
        .load::<(Uuid, String)>(conn)
        .map_err(|e| format!("failed to fetch photos: {}", e))?;
    hashes
        .into_iter()
        .map(|(id, hash)| Ok((id, live().read(&hash).map_err(|e| format!("{e:#}"))?)))
        .collect()
}

pub(super) const PAPRIKARECIPE_EXTENSION: &str = ".paprikarecipe";

/// Exported single recipe data (gzipped .paprikarecipe content).
pub(super) struct ExportedRecipe {
    pub filename: String,
    pub data: Vec<u8>,
}

/// Export a single recipe to .paprikarecipe format (gzipped JSON).
/// This is the core export function used by both single-recipe and bulk export.
pub(super) fn export_recipe_to_paprikarecipe(
    conn: &mut diesel::SqliteConnection,
    user_id: Uuid,
    recipe: &RecipeWithVersion,
) -> Result<ExportedRecipe, String> {
    let photos_data = fetch_recipe_photos(conn, user_id, &recipe.version.photo_ids)?;
    let photo_count = photos_data.len();
    let photo_bytes: usize = photos_data.iter().map(|(_, d)| d.len()).sum();

    let tags: Vec<String> = recipe_version_tags::table
        .inner_join(user_tags::table)
        .filter(recipe_version_tags::recipe_version_id.eq(recipe.version.id))
        .filter(user_tags::deleted_at.is_null())
        .select(user_tags::name)
        .order(user_tags::name.asc())
        .load(conn)
        .map_err(|e| format!("failed to fetch tags: {}", e))?;

    let paprika_recipe = convert_to_paprika(recipe, photos_data, tags)?;
    let data = gzip_recipe(&paprika_recipe)?;

    tracing::debug!(
        recipe_id = %recipe.id,
        photo_count,
        photo_bytes_raw = photo_bytes,
        gzipped_bytes = data.len(),
        "encoded .paprikarecipe"
    );

    let filename = paprika_recipe
        .name
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-' || *c == '_')
        .collect::<String>();
    let filename = filename.trim();
    let filename = if filename.is_empty() {
        "Recipe"
    } else {
        filename
    };
    let filename = format!("{}{}", filename, PAPRIKARECIPE_EXTENSION);

    Ok(ExportedRecipe { filename, data })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ramekin_core::ingredient_parser::{parse_paprika_ingredients, ParsedIngredient};
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct FormattingCase {
        name: String,
        ingredient: Ingredient,
        options: FormattingOptions,
        expected: String,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct FormattingOptions {
        #[serde(default)]
        include_alternatives: bool,
        #[serde(default)]
        include_note: bool,
        scale: Option<f64>,
        derived: Option<Measurement>,
    }

    fn ingredient(item: &str, measurements: &[(&str, &str)], section: Option<&str>) -> Ingredient {
        Ingredient {
            item: item.to_string(),
            measurements: measurements
                .iter()
                .map(|(amount, unit)| Measurement {
                    amount: Some(amount.to_string()),
                    unit: Some(unit.to_string()),
                })
                .collect(),
            note: None,
            section: section.map(str::to_string),
        }
    }

    #[test]
    fn shared_ingredient_formatting_vectors_match() {
        let cases: Vec<FormattingCase> = serde_json::from_str(include_str!(
            "../../../../shared-test-vectors/ingredient-formatting.json"
        ))
        .unwrap();

        // The export never scales or adds derived grams, so only the cases
        // without those options apply to the server.
        let mut checked = 0;
        for case in cases {
            if case.options.scale.is_some() || case.options.derived.is_some() {
                continue;
            }
            assert_eq!(
                format_ingredient_line(
                    &case.ingredient,
                    case.options.include_alternatives,
                    case.options.include_note
                ),
                case.expected,
                "{}",
                case.name
            );
            checked += 1;
        }
        assert!(checked > 0, "no applicable ingredient-formatting vectors");
    }

    #[test]
    fn ingredients_text_adds_a_header_line_per_section_change() {
        let ingredients = vec![
            ingredient("flour", &[("2", "cups"), ("240", "g")], None),
            ingredient("butter", &[("1", "stick")], Some("For the crust")),
            ingredient("water", &[("2", "tbsp")], Some("For the crust")),
            ingredient("apples", &[("6", "")], Some(" Filling ")),
            ingredient("salt", &[], Some("  ")),
        ];

        assert_eq!(
            format_ingredients_text(&ingredients),
            "2 cups (240 g) flour\n\
             For the crust:\n\
             1 stick butter\n\
             2 tbsp water\n\
             Filling:\n\
             6 apples\n\
             \n\
             salt"
        );
    }

    /// The stored shape an ingredient should come back as: trimmed, with blank
    /// measurement parts and notes dropped, and no raw text.
    fn normalized(mut ingredient: ParsedIngredient) -> ParsedIngredient {
        let clean =
            |value: Option<String>| value.as_deref().and_then(non_blank).map(str::to_string);
        ingredient.item = ingredient.item.trim().to_string();
        ingredient.measurements = ingredient
            .measurements
            .into_iter()
            .map(|m| ramekin_core::ingredient_parser::Measurement {
                amount: clean(m.amount),
                unit: clean(m.unit),
            })
            .filter(|m| m.amount.is_some() || m.unit.is_some())
            .collect();
        ingredient.note = clean(ingredient.note);
        ingredient.section = clean(ingredient.section);
        ingredient.raw = None;
        ingredient
    }

    fn round_trip(ingredients: &[Ingredient]) -> Vec<ParsedIngredient> {
        parse_paprika_ingredients(&format_ingredients_text(ingredients))
            .into_iter()
            .map(normalized)
            .collect()
    }

    fn assert_round_trips(ingredients: &[Ingredient], context: &str) {
        let expected: Vec<_> = ingredients
            .iter()
            .cloned()
            .map(|i| normalized(i.into()))
            .collect();
        assert_eq!(round_trip(ingredients), expected, "{}", context);
    }

    #[test]
    fn shared_ingredient_formatting_vectors_round_trip_through_paprika_import() {
        let cases: Vec<FormattingCase> = serde_json::from_str(include_str!(
            "../../../../shared-test-vectors/ingredient-formatting.json"
        ))
        .unwrap();

        // The export always writes alternatives and notes, so every vector's
        // stored ingredient applies, whatever display options it tests.
        for case in cases {
            // A unit-only alternative re-imports as a note; see
            // format_ingredients_text.
            if case.name == "amount-only and unit-only measurements" {
                let parsed = &round_trip(&[case.ingredient])[0];
                assert_eq!(parsed.measurements.len(), 1);
                assert_eq!(parsed.note.as_deref(), Some("pinch"));
                continue;
            }
            assert_round_trips(&[case.ingredient], &case.name);
        }
    }

    #[test]
    fn sectioned_ingredients_round_trip_through_paprika_import() {
        let mut ingredients = vec![
            ingredient("flour", &[("2", "cup"), ("240", "g")], None),
            ingredient("butter", &[("1", "stick")], Some("Part 1")),
            ingredient("water", &[("2", "tbsp")], Some("Part 1")),
            ingredient("apples", &[("6", "large")], Some("For the Filling")),
            ingredient("sugar", &[("1/2", "cup")], Some("Filling")),
            ingredient("salt", &[("1", "pinch")], None),
            ingredient("egg", &[("1", "large")], Some("Wash")),
        ];
        ingredients[2].note = Some("ice cold".to_string());
        assert_round_trips(&ingredients, "sectioned");
    }
}
