//! Harvest the server's learned ingredient names (catalog step 3) back into
//! the committed catalog: resolved rows of an `ingredient_name_resolutions`
//! export become step-2 classification decisions, to verify and apply with
//! `make catalog-apply-classification`. Once a deploy carries them, the server
//! marks their rows harvested at startup.

use crate::ingredient_catalog_audit::{read_json, LearnedRow};
use anyhow::{Context, Result};
use ramekin_core::catalog::{
    is_ambiguous, learned_key_resolves, unlearned_name, LearnedTarget, CURATED_JSON,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::sync::LazyLock;

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

static ALIASES: LazyLock<BTreeMap<String, Option<String>>> = LazyLock::new(|| {
    serde_json::from_str::<Curated>(CURATED_JSON)
        .expect("invalid catalog curated.json")
        .aliases
});

/// The name an alias for a learned key targets: the key, or the key's own
/// target when it is itself an alias, since aliases never chain.
fn alias_target(key: &str) -> Result<String> {
    Ok(match ALIASES.get(key) {
        // A resolving key is never an ambiguous (null) alias.
        Some(target) => target
            .clone()
            .context("learned key is an ambiguous alias")?,
        None => key.to_string(),
    })
}

/// A row's decision, or why it was skipped.
fn decide(row: &LearnedRow) -> Result<Result<Decision, &'static str>> {
    if row.status != "resolved" {
        return Ok(Err("not resolved"));
    }
    if unlearned_name(&row.name).is_none() {
        return Ok(Err("already in the catalog"));
    }
    if is_ambiguous(&row.name) {
        // Ambiguous on purpose: the answer is an assumption estimates label.
        return Ok(Err("ambiguous in the catalog"));
    }
    let (action, target, reason) = match row.target()? {
        LearnedTarget::Entry(key) if learned_key_resolves(&key) => {
            ("alias", Some(alias_target(&key)?), None)
        }
        LearnedTarget::Entry(_) => return Ok(Err("key no longer resolves")),
        LearnedTarget::NotFood => {
            let model = row.model.as_deref().context("resolved row has no model")?;
            ("not_food", None, Some(format!("learned from {model}")))
        }
        // An estimate has no citable source, which a `food` decision needs.
        LearnedTarget::Estimate(_) => return Ok(Err("estimate")),
        LearnedTarget::Unknown => return Ok(Err("unknown")),
    };
    Ok(Ok(Decision {
        name: row.name.clone(),
        action,
        target,
        reason,
    }))
}

/// The decisions a learned-names export harvests, and how many rows each
/// skip reason left out.
pub fn harvest(rows: &[LearnedRow]) -> Result<(Vec<Decision>, BTreeMap<&'static str, usize>)> {
    let mut decisions = Vec::new();
    let mut skipped: BTreeMap<&'static str, usize> = BTreeMap::new();
    for row in rows {
        match decide(row).with_context(|| format!("learned row {:?}", row.name))? {
            Ok(decision) => decisions.push(decision),
            Err(reason) => *skipped.entry(reason).or_default() += 1,
        }
    }
    decisions.sort_by(|a, b| a.name.cmp(&b.name));
    Ok((decisions, skipped))
}

/// Write the harvest of a learned-names export to `HARVEST_FILE`.
pub fn export(root: &Path, learned: &Path) -> Result<()> {
    let rows: Vec<LearnedRow> = read_json(learned)?;
    let (decisions, skipped) = harvest(&rows)?;
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
        let (decisions, skipped) = harvest(&rows).unwrap();
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
        let (alias, target) = ALIASES
            .iter()
            .find_map(|(alias, target)| Some((alias, target.as_ref()?)))
            .unwrap();
        let rows = [row("zorblax thing", "resolved", Some("entry"), Some(alias))];
        let (decisions, _) = harvest(&rows).unwrap();
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
        let (decisions, skipped) = harvest(&rows).unwrap();
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
