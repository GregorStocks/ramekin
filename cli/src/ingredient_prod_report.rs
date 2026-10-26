//! Prod ingredient burndown report.
//!
//! Replays calorie estimates and shopping categories over the live prod
//! recipes (exported by `scripts/export-prod-ingredients.sh`) and lists every
//! ingredient line that fails, grouped by why, with the recipe it comes from.
//! The report quotes prod recipe text, so it only goes to logs/prod/.

use crate::ingredient_catalog_audit::{load_learned, normalize_name, read_json};
use anyhow::{Context, Result};
use ramekin_core::catalog::{is_non_food_with, learned_estimate, Learned};
use ramekin_core::ingredient_categorizer::categorize_with;
use ramekin_core::ingredient_parser::{parse_ingredient, ParsedIngredient};
use ramekin_core::nutrition;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

const REPORT: &str = "logs/prod/burndown.md";
const DUMP: &str = "logs/prod/burndown.json";
/// Lines quoted per name in the Markdown report; the JSON dump has them all.
const MAX_LINES: usize = 5;

const MISSING_DENSITY: &str = "Missing density for this food";
/// A curated entry without a density replaced a learned estimate that had
/// one, so the line stopped counting: the entry needs a cited density.
const DENSITY_DROPPED: &str = "Missing density for this food (its learned estimate had one)";

#[derive(Clone, Deserialize)]
struct ProdRecipe {
    id: String,
    title: String,
    servings: Option<String>,
    ingredients: Vec<ParsedIngredient>,
}

/// Why a prod line fails, in the order a fix has to get through them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
enum Group {
    /// The stored amount isn't one the estimate can read.
    DoesNotParse,
    /// The name doesn't resolve to one catalog food.
    NameUnresolved,
    /// The food resolves but this amount can't be weighed.
    CannotWeigh,
    /// Deliberately unknown: another recipe, frying oil.
    LeftUnknown,
    /// Given no amount at all: never counted, but never makes an estimate partial.
    NoAmount,
    /// A food line the shopping list files under Other. Independent of the above.
    NoCategory,
}

impl Group {
    const ALL: [Group; 6] = [
        Group::DoesNotParse,
        Group::NameUnresolved,
        Group::CannotWeigh,
        Group::LeftUnknown,
        Group::NoAmount,
        Group::NoCategory,
    ];

    fn of_reason(reason: &str) -> Group {
        match reason {
            "Missing quantity"
            | "Unsupported or missing quantity"
            | "Several ingredients share one amount" => Group::DoesNotParse,
            "No supported nutrition match" | "Ambiguous ingredient" => Group::NameUnresolved,
            "Unsupported quantity unit" | MISSING_DENSITY | DENSITY_DROPPED => Group::CannotWeigh,
            _ => Group::LeftUnknown,
        }
    }

    fn title(self) -> &'static str {
        match self {
            Group::DoesNotParse => "Does not parse",
            Group::NameUnresolved => "Name doesn't resolve to a catalog food",
            Group::CannotWeigh => "Resolves but can't be weighed",
            Group::LeftUnknown => "Left unknown on purpose",
            Group::NoAmount => "No amount (not counted, not a failure)",
            Group::NoCategory => "No shopping category",
        }
    }

    /// Whether the line makes the calorie estimate partial or insufficient.
    fn counts_against_calories(self) -> bool {
        matches!(
            self,
            Group::DoesNotParse | Group::NameUnresolved | Group::CannotWeigh | Group::LeftUnknown
        )
    }
}

#[derive(Debug, Serialize)]
struct Failure {
    group: Group,
    reason: String,
    /// Normalized ingredient name, the unit fixes are usually keyed by.
    name: String,
    /// The stored line, written out as a person would read it.
    line: String,
    recipe_id: String,
    title: String,
    /// The line's position in its recipe.
    index: usize,
    /// Whether the current parser, re-reading the line as written out above,
    /// gets it past this group. Prod stores lines as parsed when saved, so
    /// parser fixes since then never reached them. Approximate: the written-out
    /// line isn't the original text.
    fixed_by_reparse: bool,
}

#[derive(Debug, Default, PartialEq)]
struct Totals {
    recipes: usize,
    lines: usize,
    non_food: usize,
    recipes_fully_estimated: usize,
}

/// "1 cup (240 ml) flour, sifted": the first measurement, the alternatives in
/// parentheses, then the name and note.
fn line_text(ingredient: &ParsedIngredient) -> String {
    let measurements: Vec<String> = ingredient
        .measurements
        .iter()
        .map(|m| {
            [m.amount.as_deref(), m.unit.as_deref()]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|m| !m.is_empty())
        .collect();
    let mut text = String::new();
    if let Some((first, rest)) = measurements.split_first() {
        text.push_str(first);
        if !rest.is_empty() {
            write!(text, " ({})", rest.join("; ")).unwrap();
        }
        text.push(' ');
    }
    text.push_str(&ingredient.item);
    if let Some(note) = ingredient.note.as_deref().filter(|n| !n.is_empty()) {
        write!(text, ", {note}").unwrap();
    }
    text
}

fn classify(recipes: &[ProdRecipe], learned: &Learned) -> Result<(Totals, Vec<Failure>)> {
    let mut totals = Totals::default();
    let mut failures = Vec::new();
    for recipe in recipes {
        if recipe.ingredients.is_empty() {
            continue;
        }
        totals.recipes += 1;
        totals.lines += recipe.ingredients.len();
        let estimate = nutrition::estimate_with(
            &recipe.ingredients,
            recipe.servings.as_deref(),
            1.0,
            learned,
        )
        .map_err(|e| anyhow::anyhow!("Calorie estimate failed for {}: {e}", recipe.id))?;
        if estimate.unknown_ingredients.is_empty() {
            totals.recipes_fully_estimated += 1;
        }
        let mut failure = |group, reason: &str, index: usize| {
            let ingredient = &recipe.ingredients[index];
            failures.push(Failure {
                group,
                reason: reason.to_string(),
                name: normalize_name(&ingredient.item),
                line: line_text(ingredient),
                recipe_id: recipe.id.clone(),
                title: recipe.title.clone(),
                index,
                fixed_by_reparse: false,
            });
        };
        for unknown in &estimate.unknown_ingredients {
            let item = &recipe.ingredients[unknown.index].item;
            let reason = if unknown.reason == MISSING_DENSITY
                && learned_estimate(item, learned).is_some_and(|e| e.grams_per_cup.is_some())
            {
                DENSITY_DROPPED
            } else {
                &unknown.reason
            };
            failure(Group::of_reason(reason), reason, unknown.index);
        }
        for no_amount in &estimate.no_amount {
            failure(Group::NoAmount, &no_amount.reason, no_amount.index);
        }
        for (index, ingredient) in recipe.ingredients.iter().enumerate() {
            if is_non_food_with(&ingredient.item, learned) {
                totals.non_food += 1;
            } else if categorize_with(&ingredient.item, learned) == "Other" {
                failure(Group::NoCategory, "Other", index);
            }
        }
    }
    Ok((totals, failures))
}

/// The recipes with every line written out and parsed again by the current parser.
fn reparsed(recipes: &[ProdRecipe]) -> Vec<ProdRecipe> {
    recipes
        .iter()
        .map(|recipe| ProdRecipe {
            ingredients: recipe
                .ingredients
                .iter()
                .map(|ingredient| ParsedIngredient {
                    section: ingredient.section.clone(),
                    ..parse_ingredient(&line_text(ingredient))
                })
                .collect(),
            ..recipe.clone()
        })
        .collect()
}

/// `classify`, marking each failure the current parser would get past. A
/// calorie failure (or no amount) is fixed only if the re-parsed line counts
/// toward calories, so a fix that exposes a later group isn't a fix yet.
fn classify_with_reparse(
    recipes: &[ProdRecipe],
    learned: &Learned,
) -> Result<(Totals, Vec<Failure>)> {
    let (totals, mut failures) = classify(recipes, learned)?;
    let (_, after) = classify(&reparsed(recipes), learned)?;
    let still_failing: BTreeSet<(&str, usize, bool)> = after
        .iter()
        .map(|f| (f.recipe_id.as_str(), f.index, f.group == Group::NoCategory))
        .collect();
    for failure in &mut failures {
        failure.fixed_by_reparse = !still_failing.contains(&(
            failure.recipe_id.as_str(),
            failure.index,
            failure.group == Group::NoCategory,
        ));
    }
    Ok((totals, failures))
}

/// Names in one group and reason, most lines first.
fn by_name<'a>(failures: &[&'a Failure]) -> Vec<(&'a str, Vec<&'a Failure>)> {
    let mut names: BTreeMap<&str, Vec<&Failure>> = BTreeMap::new();
    for failure in failures {
        names.entry(&failure.name).or_default().push(failure);
    }
    let mut names: Vec<_> = names.into_iter().collect();
    names.sort_by(|(a, a_lines), (b, b_lines)| b_lines.len().cmp(&a_lines.len()).then(a.cmp(b)));
    names
}

fn recipe_count<'a>(failures: impl IntoIterator<Item = &'a &'a Failure>) -> usize {
    failures
        .into_iter()
        .map(|f| f.recipe_id.as_str())
        .collect::<BTreeSet<_>>()
        .len()
}

/// Counts only, no recipe text: safe to paste into PRs and issues.
fn render_summary(totals: &Totals, failures: &[Failure]) -> String {
    let mut out = String::new();
    writeln!(
        out,
        "{} recipes, {} lines ({} non-food); {} recipes fully estimated.\n",
        totals.recipes, totals.lines, totals.non_food, totals.recipes_fully_estimated
    )
    .unwrap();
    writeln!(
        out,
        "| Group | Lines | Names | Recipes | Fixed by re-parsing |"
    )
    .unwrap();
    writeln!(out, "|---|---:|---:|---:|---:|").unwrap();
    for group in Group::ALL {
        let lines: Vec<&Failure> = failures.iter().filter(|f| f.group == group).collect();
        let names = lines.iter().map(|f| &f.name).collect::<BTreeSet<_>>().len();
        let reparse = lines.iter().filter(|f| f.fixed_by_reparse).count();
        writeln!(
            out,
            "| {} | {} | {} | {} | {} |",
            group.title(),
            lines.len(),
            names,
            recipe_count(&lines),
            reparse
        )
        .unwrap();
    }
    let unknown = failures
        .iter()
        .filter(|f| f.group.counts_against_calories())
        .count();
    writeln!(
        out,
        "\n{unknown} lines keep a calorie estimate from being complete."
    )
    .unwrap();
    out
}

fn render_report(totals: &Totals, failures: &[Failure]) -> String {
    let mut out = String::from(
        "# Prod ingredient burndown\n\n\
         Generated by `make ingredient-prod-report` from `make prod-export`. \
         Quotes prod recipes: keep it out of git.\n\n",
    );
    out.push_str(&render_summary(totals, failures));
    for group in Group::ALL {
        let lines: Vec<&Failure> = failures.iter().filter(|f| f.group == group).collect();
        writeln!(out, "\n## {} ({} lines)", group.title(), lines.len()).unwrap();
        let reasons: BTreeSet<&str> = lines.iter().map(|f| f.reason.as_str()).collect();
        for reason in reasons {
            let reason_lines: Vec<&Failure> = lines
                .iter()
                .copied()
                .filter(|f| f.reason == reason)
                .collect();
            writeln!(out, "\n### {reason} ({} lines)\n", reason_lines.len()).unwrap();
            for (name, name_lines) in by_name(&reason_lines) {
                writeln!(
                    out,
                    "- **{name}**: {} lines, {} recipes",
                    name_lines.len(),
                    recipe_count(&name_lines)
                )
                .unwrap();
                for failure in name_lines.iter().take(MAX_LINES) {
                    let reparse = if failure.fixed_by_reparse {
                        " — fixed by re-parsing"
                    } else {
                        ""
                    };
                    writeln!(out, "  - `{}` ({}){reparse}", failure.line, failure.title).unwrap();
                }
                if name_lines.len() > MAX_LINES {
                    writeln!(out, "  - …and {} more", name_lines.len() - MAX_LINES).unwrap();
                }
            }
        }
    }
    out
}

pub fn run(root: &Path, prod_recipes: &Path, learned: Option<&Path>) -> Result<()> {
    let recipes: Vec<ProdRecipe> = read_json(prod_recipes)?;
    let learned = learned.map(load_learned).transpose()?.unwrap_or_default();
    let (totals, failures) = classify_with_reparse(&recipes, &learned)?;

    let report_path = root.join(REPORT);
    fs::create_dir_all(report_path.parent().unwrap())?;
    fs::write(&report_path, render_report(&totals, &failures))
        .with_context(|| format!("Failed to write {}", report_path.display()))?;
    let dump_path = root.join(DUMP);
    fs::write(&dump_path, serde_json::to_string_pretty(&failures)? + "\n")
        .with_context(|| format!("Failed to write {}", dump_path.display()))?;

    tracing::info!("{}", render_summary(&totals, &failures));
    tracing::info!("Every failing prod line saved to: {REPORT} and {DUMP}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ramekin_core::catalog::{EstimatedFood, LearnedTarget};

    fn recipe(ingredients: serde_json::Value) -> ProdRecipe {
        ProdRecipe {
            id: "r1".to_string(),
            title: "Test recipe".to_string(),
            servings: None,
            ingredients: serde_json::from_value(ingredients).unwrap(),
        }
    }

    fn groups(failures: &[Failure]) -> Vec<(Group, &str)> {
        failures
            .iter()
            .map(|f| (f.group, f.name.as_str()))
            .collect()
    }

    #[test]
    fn each_line_lands_in_its_group() {
        let recipes = [recipe(serde_json::json!([
            {"item": "granulated sugar", "measurements": [{"amount": "100", "unit": "g"}]},
            {"item": "granulated sugar", "measurements": [{"amount": "some", "unit": "cup"}]},
            {"item": "Zzqq Moon Dust", "measurements": [{"amount": "1", "unit": "cup"}]},
            {"item": "granulated sugar", "measurements": [{"amount": "2", "unit": "zzqq"}]},
            {"item": "olive oil", "measurements": []},
            {"item": "parchment paper", "measurements": []},
        ]))];
        let (totals, failures) = classify(&recipes, &Learned::new()).unwrap();
        assert_eq!(
            totals,
            Totals {
                recipes: 1,
                lines: 6,
                non_food: 1,
                recipes_fully_estimated: 0,
            }
        );
        assert_eq!(
            groups(&failures),
            [
                (Group::DoesNotParse, "granulated sugar"),
                (Group::NameUnresolved, "zzqq moon dust"),
                (Group::CannotWeigh, "granulated sugar"),
                (Group::NoAmount, "olive oil"),
                (Group::NoCategory, "zzqq moon dust"),
            ]
        );
        assert_eq!(failures[1].line, "1 cup Zzqq Moon Dust");
        assert_eq!(failures[1].title, "Test recipe");
    }

    #[test]
    fn a_density_a_learned_estimate_had_is_called_out() {
        let recipes = [recipe(serde_json::json!([
            {"item": "chestnut spread", "measurements": [{"amount": "3", "unit": "cups"}]},
            {"item": "pickled onions", "measurements": [{"amount": "2", "unit": "cups"}]},
        ]))];
        let estimate = |grams_per_cup| {
            LearnedTarget::Estimate(EstimatedFood {
                kcal_per_100g: 100.0,
                grams_per_cup,
                grams_per_piece: None,
                category: None,
            })
        };
        let learned = Learned::from([
            ("chestnut spread".to_string(), estimate(Some(240.0))),
            ("pickled onions".to_string(), estimate(None)),
        ]);
        let (_, failures) = classify(&recipes, &learned).unwrap();
        let reasons: Vec<_> = failures
            .iter()
            .map(|f| (f.name.as_str(), f.group, f.reason.as_str()))
            .collect();
        assert_eq!(
            reasons,
            [
                ("chestnut spread", Group::CannotWeigh, DENSITY_DROPPED),
                ("pickled onions", Group::CannotWeigh, MISSING_DENSITY),
            ]
        );
    }

    #[test]
    fn reparse_marks_lines_the_current_parser_fixes() {
        // Stored by an older parser: the amount left in the name.
        let recipes = [recipe(serde_json::json!([
            {"item": "Juice of half a lemon", "measurements": []},
            {"item": "olive oil", "measurements": []},
        ]))];
        let (_, failures) = classify_with_reparse(&recipes, &Learned::new()).unwrap();
        let fixed: Vec<_> = failures
            .iter()
            .map(|f| (f.name.as_str(), f.fixed_by_reparse))
            .collect();
        assert_eq!(
            fixed,
            [("juice of half a lemon", true), ("olive oil", false)]
        );
    }

    #[test]
    fn line_text_reads_like_the_recipe() {
        let ingredient: ParsedIngredient = serde_json::from_value(serde_json::json!({
            "item": "flour",
            "measurements": [{"amount": "1", "unit": "cup"}, {"amount": "120", "unit": "g"}],
            "note": "sifted",
        }))
        .unwrap();
        assert_eq!(line_text(&ingredient), "1 cup (120 g) flour, sifted");
    }
}
