//! Harvest the server's learned ingredient names (catalog step 3) back into
//! the committed catalog: resolved rows of an `ingredient_name_resolutions`
//! export become step-2 classification decisions, to verify and apply with
//! `make catalog-apply-classification`. Once a deploy carries them, the server
//! marks their rows harvested at startup.

use crate::ingredient_catalog_audit::{read_json, LearnedRow, CURATED_PATH};
use anyhow::{Context, Result};
use ramekin_core::catalog::{is_ambiguous, learned_key_resolves, unlearned_name};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

const HARVEST_FILE: &str = "logs/catalog-harvest-learned.json";

/// A classification decision, as `scripts/apply-catalog-classification.py`
/// reads it.
#[derive(Debug, PartialEq, Serialize)]
pub struct Decision {
    pub name: String,
    pub action: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Deserialize)]
struct Curated {
    /// Alias name to target; `None` marks an ambiguous name.
    aliases: BTreeMap<String, Option<String>>,
}

/// The decisions a learned-names export harvests, and how many rows each
/// skip reason left out. `aliases` are `curated.json`'s, so a learned key
/// that is itself an alias becomes its target (aliases never chain).
pub fn harvest(
    rows: &[LearnedRow],
    aliases: &BTreeMap<String, Option<String>>,
) -> Result<(Vec<Decision>, BTreeMap<&'static str, usize>)> {
    let mut decisions = Vec::new();
    let mut skipped: BTreeMap<&'static str, usize> = BTreeMap::new();
    for row in rows {
        let skip = if row.status != "resolved" {
            Some("not resolved")
        } else if unlearned_name(&row.name).is_none() {
            Some("already in the catalog")
        } else if is_ambiguous(&row.name) {
            // Ambiguous on purpose: the answer is an assumption estimates label.
            Some("ambiguous in the catalog")
        } else {
            None
        };
        if let Some(reason) = skip {
            *skipped.entry(reason).or_default() += 1;
            continue;
        }
        let decision = match (row.disposition.as_deref(), row.catalog_key.as_deref()) {
            (Some("entry"), Some(key)) if learned_key_resolves(key) => {
                let target = match aliases.get(key) {
                    Some(Some(target)) => target.clone(),
                    Some(None) => anyhow::bail!("learned key {key:?} is an ambiguous alias"),
                    None => key.to_string(),
                };
                Decision {
                    name: row.name.clone(),
                    action: "alias",
                    target: Some(target),
                    reason: None,
                }
            }
            (Some("entry"), _) => {
                *skipped.entry("key no longer resolves").or_default() += 1;
                continue;
            }
            (Some("not_food"), _) => {
                let model = row
                    .model
                    .as_deref()
                    .with_context(|| format!("resolved row {:?} has no model", row.name))?;
                Decision {
                    name: row.name.clone(),
                    action: "not_food",
                    target: None,
                    reason: Some(format!("learned from {model}")),
                }
            }
            // An estimate has no citable source, which a `food` decision needs.
            (Some("estimate"), _) => {
                *skipped.entry("estimate").or_default() += 1;
                continue;
            }
            (Some("unknown"), _) => {
                *skipped.entry("unknown").or_default() += 1;
                continue;
            }
            (disposition, _) => {
                anyhow::bail!(
                    "bad learned row {:?}: disposition {disposition:?}",
                    row.name
                )
            }
        };
        decisions.push(decision);
    }
    decisions.sort_by(|a, b| a.name.cmp(&b.name));
    Ok((decisions, skipped))
}

/// Write the harvest of a learned-names export to `HARVEST_FILE`.
pub fn export(root: &Path, learned: &Path) -> Result<()> {
    let rows: Vec<LearnedRow> = read_json(learned)?;
    let curated: Curated = read_json(&root.join(CURATED_PATH))?;
    let (decisions, skipped) = harvest(&rows, &curated.aliases)?;
    let path = root.join(HARVEST_FILE);
    fs::create_dir_all(root.join("logs"))?;
    fs::write(&path, serde_json::to_string_pretty(&decisions)? + "\n")
        .with_context(|| format!("Failed to write {}", path.display()))?;
    tracing::info!(
        "{} decisions saved to {HARVEST_FILE}; skipped {skipped:?}",
        decisions.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(name: &str, status: &str, disposition: Option<&str>, key: Option<&str>) -> LearnedRow {
        serde_json::from_value(serde_json::json!({
            "name": name,
            "status": status,
            "disposition": disposition,
            "catalog_key": key,
            "model": "test-model",
        }))
        .unwrap()
    }

    fn curated_aliases() -> BTreeMap<String, Option<String>> {
        let curated: Curated = read_json(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join(CURATED_PATH),
        )
        .unwrap();
        curated.aliases
    }

    #[test]
    fn entries_become_aliases_and_non_foods_stay_non_foods() {
        let rows = [
            row(
                "zorblax flour",
                "resolved",
                Some("entry"),
                Some("all-purpose flour"),
            ),
            row("for the zorblax", "resolved", Some("not_food"), None),
        ];
        let (decisions, skipped) = harvest(&rows, &curated_aliases()).unwrap();
        assert_eq!(
            decisions,
            [
                Decision {
                    name: "for the zorblax".into(),
                    action: "not_food",
                    target: None,
                    reason: Some("learned from test-model".into()),
                },
                Decision {
                    name: "zorblax flour".into(),
                    action: "alias",
                    target: Some("all-purpose flour".into()),
                    reason: None,
                },
            ]
        );
        assert!(skipped.is_empty());
    }

    #[test]
    fn an_alias_key_becomes_its_target() {
        let aliases = curated_aliases();
        let (alias, target) = aliases
            .iter()
            .find_map(|(alias, target)| Some((alias, target.as_ref()?)))
            .unwrap();
        let rows = [row("zorblax thing", "resolved", Some("entry"), Some(alias))];
        let (decisions, _) = harvest(&rows, &aliases).unwrap();
        assert_eq!(decisions[0].target.as_ref(), Some(target));
    }

    #[test]
    fn rows_that_cannot_be_harvested_are_counted() {
        let rows = [
            row("zorblax a", "pending", None, None),
            row(
                "all-purpose flour",
                "resolved",
                Some("entry"),
                Some("all-purpose flour"),
            ),
            row("cheese", "resolved", Some("entry"), Some("cheddar cheese")),
            row(
                "zorblax b",
                "resolved",
                Some("entry"),
                Some("no such zorblax"),
            ),
            row("zorblax c", "resolved", Some("unknown"), None),
            serde_json::from_value(serde_json::json!({
                "name": "zorblax d",
                "status": "resolved",
                "disposition": "estimate",
                "catalog_key": null,
                "kcal_per_100g": 120.0,
            }))
            .unwrap(),
        ];
        let (decisions, skipped) = harvest(&rows, &curated_aliases()).unwrap();
        assert!(decisions.is_empty());
        assert_eq!(
            skipped,
            BTreeMap::from([
                ("already in the catalog", 1),
                ("ambiguous in the catalog", 1),
                ("estimate", 1),
                ("key no longer resolves", 1),
                ("not resolved", 1),
                ("unknown", 1),
            ])
        );
    }
}
