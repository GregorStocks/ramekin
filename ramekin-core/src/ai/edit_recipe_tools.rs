//! The recipe an Edit with AI run is changing, and the tools the model edits
//! it with. Edits change only what they name, so the rest of the recipe stays
//! byte-identical.

use async_trait::async_trait;
use serde_json::{json, Map, Value};

use crate::ai::ToolSpec;
use crate::catalog::{self, Resolution};
use crate::ingredient_parser::parse_ingredient;
use crate::metric_weights::parse_amount;

/// The user's data the tools can look at, which only the server has.
#[async_trait]
pub trait RecipeEditContext: Send + Sync {
    /// The user's tags.
    async fn user_tags(&self) -> Result<Vec<String>, String>;
    /// The user's recipes best matching `query`: a few, each as text.
    async fn search_recipes(&self, query: &str) -> Result<Vec<String>, String>;
}

/// No user data: for evals, which edit a recipe on its own.
pub struct NoUserData;

#[async_trait]
impl RecipeEditContext for NoUserData {
    async fn user_tags(&self) -> Result<Vec<String>, String> {
        Ok(Vec::new())
    }

    async fn search_recipes(&self, _query: &str) -> Result<Vec<String>, String> {
        Ok(Vec::new())
    }
}

/// Fields `set_field` can change, as the recipe JSON names them.
const TEXT_FIELDS: &[&str] = &[
    "title",
    "description",
    "notes",
    "servings",
    "prep_time",
    "cook_time",
    "total_time",
    "difficulty",
    "nutritional_info",
];

/// The tool that ends the run.
pub const FINISH_TOOL: &str = "finish";

fn tool(name: &str, description: &str, properties: Value, required: &[&str]) -> ToolSpec {
    ToolSpec {
        name: name.to_string(),
        description: description.to_string(),
        parameters: json!({
            "type": "object",
            "properties": properties,
            "required": required,
        }),
    }
}

/// Every tool an Edit with AI run can call.
pub fn edit_recipe_tools() -> Vec<ToolSpec> {
    vec![
        tool(
            "lookup_ingredient",
            "Look an ingredient up in the catalog: its calories per 100 g, grams per cup, and grams per piece (e.g. per clove or per large egg). Use it for unit conversions and calorie questions instead of guessing.",
            json!({"name": {"type": "string", "description": "The ingredient, e.g. \"all-purpose flour\""}}),
            &["name"],
        ),
        tool(
            "scale_recipe",
            "Multiply every ingredient amount by a factor (0.5 halves the recipe). Servings are not changed; set them with set_field.",
            json!({"factor": {"type": "number", "description": "The multiplier, greater than 0"}}),
            &["factor"],
        ),
        tool(
            "list_tags",
            "List the user's existing tags, to reuse them rather than invent new ones.",
            json!({}),
            &[],
        ),
        tool(
            "search_recipes",
            "Search the user's other recipes, e.g. to follow \"make the sauce like my other one\".",
            json!({"query": {"type": "string", "description": "Words to search titles and ingredients for"}}),
            &["query"],
        ),
        tool(
            "set_ingredient",
            "Replace ingredient N with a new line, written as a recipe would (\"2 cups flour, sifted\"). It is parsed into amount, unit, item and note.",
            json!({
                "index": {"type": "number", "description": "The ingredient's number, from 1"},
                "line": {"type": "string", "description": "The new ingredient line"}
            }),
            &["index", "line"],
        ),
        tool(
            "add_ingredient",
            "Add an ingredient line after ingredient N (0 adds it first), in that ingredient's section.",
            json!({
                "after": {"type": "number", "description": "The ingredient number to add after, or 0"},
                "line": {"type": "string", "description": "The new ingredient line"}
            }),
            &["after", "line"],
        ),
        tool(
            "remove_ingredient",
            "Remove ingredient N.",
            json!({"index": {"type": "number", "description": "The ingredient's number, from 1"}}),
            &["index"],
        ),
        tool(
            "move_ingredient",
            "Move ingredient N to after ingredient M (0 moves it first). Numbers are as they are before the move; it keeps its section.",
            json!({
                "index": {"type": "number", "description": "The ingredient's number, from 1"},
                "after": {"type": "number", "description": "The ingredient number to move it after, or 0"}
            }),
            &["index", "after"],
        ),
        tool(
            "set_section",
            "Put ingredients FROM through TO in a section (a heading like \"For the sauce\"), or \"\" to take them out of any section.",
            json!({
                "from": {"type": "number", "description": "The first ingredient's number, from 1"},
                "to": {"type": "number", "description": "The last ingredient's number"},
                "section": {"type": "string", "description": "The section's heading, or \"\""}
            }),
            &["from", "to", "section"],
        ),
        tool(
            "set_step",
            "Replace step N of the instructions with new text.",
            json!({
                "index": {"type": "number", "description": "The step's number, from 1"},
                "text": {"type": "string", "description": "The new step"}
            }),
            &["index", "text"],
        ),
        tool(
            "add_step",
            "Add a step after step N (0 adds it first).",
            json!({
                "after": {"type": "number", "description": "The step number to add after, or 0"},
                "text": {"type": "string", "description": "The new step"}
            }),
            &["after", "text"],
        ),
        tool(
            "remove_step",
            "Remove step N.",
            json!({"index": {"type": "number", "description": "The step's number, from 1"}}),
            &["index"],
        ),
        tool(
            "set_field",
            &format!(
                "Set one of the recipe's fields ({}) to new text, or to \"\" to clear it.",
                TEXT_FIELDS.join(", ")
            ),
            json!({
                "field": {"type": "string", "description": "The field to set"},
                "value": {"type": "string", "description": "Its new text"}
            }),
            &["field", "value"],
        ),
        tool(
            "add_tags",
            "Add tags to the recipe (it keeps the ones it has). Prefer the user's existing tags from list_tags.",
            json!({"tags": {"type": "array", "items": {"type": "string"}, "description": "The tags to add"}}),
            &["tags"],
        ),
        tool(
            FINISH_TOOL,
            "Finish once the recipe says what the user asked for. Call it alone, after your edits.",
            json!({"summary": {"type": "string", "description": "One or two sentences on what you changed, for the user"}}),
            &["summary"],
        ),
    ]
}

/// The recipe a run is editing, as the caller's JSON object.
pub struct RecipeDraft {
    recipe: Map<String, Value>,
}

impl RecipeDraft {
    pub fn new(recipe: Value) -> Result<Self, String> {
        match recipe {
            Value::Object(recipe) => Ok(Self { recipe }),
            other => Err(format!("A recipe must be a JSON object, not {other}")),
        }
    }

    pub fn into_value(self) -> Value {
        Value::Object(self.recipe)
    }

    fn text(&self, field: &str) -> &str {
        self.recipe.get(field).and_then(Value::as_str).unwrap_or("")
    }

    pub(crate) fn ingredients(&self) -> &[Value] {
        self.recipe
            .get("ingredients")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    fn ingredients_mut(&mut self) -> &mut Vec<Value> {
        let entry = self
            .recipe
            .entry("ingredients")
            .or_insert_with(|| Value::Array(Vec::new()));
        if !entry.is_array() {
            *entry = Value::Array(Vec::new());
        }
        entry
            .as_array_mut()
            .expect("ingredients was just made an array")
    }

    /// The instructions' lines, and the indexes of the non-blank ones, which
    /// are the steps.
    fn steps(&self) -> (Vec<String>, Vec<usize>) {
        let lines: Vec<String> = self
            .text("instructions")
            .split('\n')
            .map(str::to_string)
            .collect();
        let steps = lines
            .iter()
            .enumerate()
            .filter(|(_, line)| !line.trim().is_empty())
            .map(|(index, _)| index)
            .collect();
        (lines, steps)
    }

    fn set_instructions(&mut self, lines: &[String]) {
        self.recipe
            .insert("instructions".to_string(), Value::String(lines.join("\n")));
    }

    /// The recipe as the model reads it, ingredients and steps numbered.
    pub fn render(&self) -> String {
        let mut out = format!("Title: {}\n", self.text("title"));
        for (label, field) in [
            ("Description", "description"),
            ("Servings", "servings"),
            ("Prep time", "prep_time"),
            ("Cook time", "cook_time"),
            ("Total time", "total_time"),
            ("Difficulty", "difficulty"),
            ("Nutritional info", "nutritional_info"),
        ] {
            let value = self.text(field);
            if !value.is_empty() {
                out.push_str(&format!("{label}: {value}\n"));
            }
        }
        let tags: Vec<&str> = self
            .recipe
            .get("tags")
            .and_then(Value::as_array)
            .map(|tags| tags.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        if !tags.is_empty() {
            out.push_str(&format!("Tags: {}\n", tags.join(", ")));
        }

        out.push_str("\nIngredients:\n");
        let mut section: Option<&str> = None;
        for (index, ingredient) in self.ingredients().iter().enumerate() {
            let this_section = ingredient.get("section").and_then(Value::as_str);
            if this_section != section {
                section = this_section;
                if let Some(name) = section {
                    out.push_str(&format!("  [{name}]\n"));
                }
            }
            out.push_str(&format!(
                "  {}. {}\n",
                index + 1,
                ingredient_line(ingredient)
            ));
        }

        out.push_str("\nSteps:\n");
        let (lines, steps) = self.steps();
        for (number, line) in steps.iter().enumerate() {
            out.push_str(&format!("  {}. {}\n", number + 1, lines[*line].trim()));
        }

        let notes = self.text("notes");
        if !notes.is_empty() {
            out.push_str(&format!("\nNotes: {notes}\n"));
        }
        out
    }

    /// Run a recipe-changing tool, returning what to tell the model.
    fn edit(&mut self, name: &str, args: &Value) -> Result<String, String> {
        match name {
            "scale_recipe" => {
                let factor = number_arg(args, "factor")?;
                if !(factor > 0.0 && factor.is_finite()) {
                    return Err("The factor must be greater than 0".to_string());
                }
                let mut scaled = 0;
                for ingredient in self.ingredients_mut() {
                    let Some(measurements) = ingredient
                        .get_mut("measurements")
                        .and_then(Value::as_array_mut)
                    else {
                        continue;
                    };
                    for measurement in measurements {
                        // A per-item size ("10 oz each") stays as it is;
                        // the count it describes scales.
                        if is_per_item(measurement) {
                            continue;
                        }
                        let Some(amount) = measurement
                            .get("amount")
                            .and_then(Value::as_str)
                            .and_then(parse_amount)
                        else {
                            continue;
                        };
                        measurement["amount"] = Value::String(format_amount(amount * factor));
                        scaled += 1;
                    }
                }
                Ok(format!(
                    "Scaled {scaled} amounts by {factor}. Amounts that aren't plain numbers (\"2-3\", \"a pinch\") are unchanged; fix any with set_ingredient. The ingredients now:\n{}",
                    self.render_ingredients()
                ))
            }
            "set_ingredient" => {
                let index = index_arg(args, "index", self.ingredients().len())?;
                let line = string_arg(args, "line")?;
                let ingredients = self.ingredients_mut();
                let section = ingredients[index].get("section").cloned();
                ingredients[index] = parsed_ingredient(&line, section)?;
                Ok(format!(
                    "Ingredient {} is now: {}",
                    index + 1,
                    ingredient_line(&ingredients[index])
                ))
            }
            "add_ingredient" => {
                let count = self.ingredients().len();
                let after = position_arg(args, "after", count)?;
                let line = string_arg(args, "line")?;
                let ingredients = self.ingredients_mut();
                // In the section of the ingredient it follows (or precedes,
                // when it goes first).
                let neighbor = if after > 0 { after - 1 } else { 0 };
                let section = ingredients
                    .get(neighbor)
                    .and_then(|ingredient| ingredient.get("section").cloned());
                ingredients.insert(after, parsed_ingredient(&line, section)?);
                Ok(format!(
                    "Added ingredient {}: {}",
                    after + 1,
                    ingredient_line(&ingredients[after])
                ))
            }
            "remove_ingredient" => {
                let index = index_arg(args, "index", self.ingredients().len())?;
                let removed = self.ingredients_mut().remove(index);
                Ok(format!(
                    "Removed ingredient {}: {}. Later ingredients moved up one number.",
                    index + 1,
                    ingredient_line(&removed)
                ))
            }
            "move_ingredient" => {
                let count = self.ingredients().len();
                let index = index_arg(args, "index", count)?;
                let after = position_arg(args, "after", count)?;
                let ingredients = self.ingredients_mut();
                let moved = ingredients.remove(index);
                // `after` counts the moved ingredient when it came before.
                let at = if after > index { after - 1 } else { after };
                ingredients.insert(at, moved);
                Ok(format!(
                    "Moved it to number {}. The ingredients now:\n{}",
                    at + 1,
                    self.render_ingredients()
                ))
            }
            "set_section" => {
                let count = self.ingredients().len();
                let from = index_arg(args, "from", count)?;
                let to = index_arg(args, "to", count)?;
                if to < from {
                    return Err("to must not be before from".to_string());
                }
                let section = string_arg(args, "section")?;
                let section = match section.trim() {
                    "" => Value::Null,
                    name => Value::String(one_line(name)?),
                };
                for ingredient in &mut self.ingredients_mut()[from..=to] {
                    ingredient["section"] = section.clone();
                }
                Ok(format!(
                    "Set the section of ingredients {} to {}.",
                    from + 1,
                    to + 1
                ))
            }
            "set_step" => {
                let (mut lines, steps) = self.steps();
                let step = index_arg(args, "index", steps.len())?;
                let text = one_line(&string_arg(args, "text")?)?;
                lines[steps[step]] = text;
                self.set_instructions(&lines);
                Ok(format!("Step {} is now: {}", step + 1, lines[steps[step]]))
            }
            "add_step" => {
                let (mut lines, steps) = self.steps();
                let after = position_arg(args, "after", steps.len())?;
                let text = one_line(&string_arg(args, "text")?)?;
                // Keep the instructions' spacing: a blank line between steps
                // if they have one.
                let blank_between = self.text("instructions").contains("\n\n");
                let mut new_lines = vec![text];
                if lines.iter().all(|line| line.trim().is_empty()) {
                    lines = new_lines;
                } else if after == 0 {
                    if blank_between {
                        new_lines.push(String::new());
                    }
                    let at = steps[0];
                    lines.splice(at..at, new_lines);
                } else {
                    if blank_between {
                        new_lines.insert(0, String::new());
                    }
                    let at = steps[after - 1] + 1;
                    lines.splice(at..at, new_lines);
                }
                self.set_instructions(&lines);
                Ok(format!(
                    "Added step {}. Later steps moved down one number.",
                    after + 1
                ))
            }
            "remove_step" => {
                let (mut lines, steps) = self.steps();
                let step = index_arg(args, "index", steps.len())?;
                let line = steps[step];
                let removed = lines.remove(line);
                // Drop the blank line that separated it, if any.
                if line < lines.len() && lines[line].trim().is_empty() {
                    lines.remove(line);
                } else if line > 0 && lines[line - 1].trim().is_empty() {
                    lines.remove(line - 1);
                }
                self.set_instructions(&lines);
                Ok(format!(
                    "Removed step {}: {}. Later steps moved up one number.",
                    step + 1,
                    removed.trim()
                ))
            }
            "set_field" => {
                let field = string_arg(args, "field")?;
                if !TEXT_FIELDS.contains(&field.as_str()) {
                    return Err(format!(
                        "{field} isn't a field set_field can change; it can change {}",
                        TEXT_FIELDS.join(", ")
                    ));
                }
                let value = string_arg(args, "value")?;
                let value = if value.trim().is_empty() {
                    Value::Null
                } else {
                    Value::String(value)
                };
                if field == "title" && value.is_null() {
                    return Err("A recipe needs a title".to_string());
                }
                self.recipe.insert(field.clone(), value);
                Ok(format!("Set {field}."))
            }
            "add_tags" => {
                let new_tags: Vec<String> = args
                    .get("tags")
                    .and_then(Value::as_array)
                    .ok_or("tags must be a list of strings")?
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::trim)
                    .filter(|tag| !tag.is_empty())
                    .map(str::to_string)
                    .collect();
                let mut tags: Vec<String> = self
                    .recipe
                    .get("tags")
                    .and_then(Value::as_array)
                    .map(|tags| {
                        tags.iter()
                            .filter_map(Value::as_str)
                            .map(str::to_string)
                            .collect()
                    })
                    .unwrap_or_default();
                for tag in new_tags {
                    if !tags
                        .iter()
                        .any(|existing| existing.eq_ignore_ascii_case(&tag))
                    {
                        tags.push(tag);
                    }
                }
                self.recipe.insert("tags".to_string(), json!(tags));
                Ok(format!("The tags are now: {}", tags.join(", ")))
            }
            other => Err(format!("There is no tool called {other}")),
        }
    }

    fn render_ingredients(&self) -> String {
        self.ingredients()
            .iter()
            .enumerate()
            .map(|(index, ingredient)| format!("  {}. {}", index + 1, ingredient_line(ingredient)))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// A recipe (the server's recipe JSON) as the model reads it, for
/// `search_recipes` results.
pub fn render_recipe(recipe: Value) -> String {
    match RecipeDraft::new(recipe) {
        Ok(draft) => draft.render(),
        Err(e) => e,
    }
}

/// Run tool `name` with the model's `arguments`, returning what to tell the
/// model: the result, or what was wrong, so it can try again.
pub async fn run_tool(
    draft: &mut RecipeDraft,
    context: &dyn RecipeEditContext,
    name: &str,
    arguments: &str,
) -> String {
    let args: Value = match serde_json::from_str(arguments) {
        Ok(Value::Object(args)) => Value::Object(args),
        Ok(_) | Err(_) if arguments.trim().is_empty() => json!({}),
        Ok(other) => return format!("Error: the arguments must be a JSON object, not {other}"),
        Err(e) => return format!("Error: the arguments aren't valid JSON: {e}"),
    };
    let result = match name {
        "lookup_ingredient" => string_arg(&args, "name").map(|name| lookup_ingredient(&name)),
        "list_tags" => context.user_tags().await.map(|tags| {
            if tags.is_empty() {
                "The user has no tags yet.".to_string()
            } else {
                tags.join(", ")
            }
        }),
        "search_recipes" => match string_arg(&args, "query") {
            Ok(query) => context.search_recipes(&query).await.map(|recipes| {
                if recipes.is_empty() {
                    "No recipes match.".to_string()
                } else {
                    recipes.join("\n\n---\n\n")
                }
            }),
            Err(e) => Err(e),
        },
        _ => draft.edit(name, &args),
    };
    result.unwrap_or_else(|e| format!("Error: {e}"))
}

/// What the catalog knows about an ingredient, as text for the model.
fn lookup_ingredient(name: &str) -> String {
    match catalog::resolve_line(name, None) {
        Resolution::Entry { entry, .. } => entry_json(entry).to_string(),
        Resolution::Compound(entries) => {
            Value::Array(entries.into_iter().map(entry_json).collect()).to_string()
        }
        Resolution::NotFood => format!("\"{name}\" isn't a food."),
        Resolution::Ambiguous(ambiguous) => format!(
            "\"{}\" could mean several foods; look up a more specific name.",
            ambiguous.name
        ),
        Resolution::Unresolved => format!(
            "The catalog doesn't know \"{name}\". Try another name, or use your own knowledge and say so in the summary."
        ),
    }
}

fn entry_json(entry: &catalog::Entry) -> Value {
    json!({
        "food": entry.id,
        "kcal_per_100g": entry.kcal_per_100g,
        "grams_per_cup": entry.grams_per_cup,
        "grams_per_piece": entry.portions,
        "a_bare_count_means": entry.default_portion,
    })
}

/// An ingredient as a recipe line: "1.5 cup (180 g) flour, sifted".
fn ingredient_line(ingredient: &Value) -> String {
    let measurement = |m: &Value| {
        [m.get("amount"), m.get("unit")]
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    };
    let measurements: Vec<String> = ingredient
        .get("measurements")
        .and_then(Value::as_array)
        .map(|ms| {
            ms.iter()
                .map(measurement)
                .filter(|m| !m.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let item = ingredient.get("item").and_then(Value::as_str).unwrap_or("");
    let mut line = match measurements.split_first() {
        Some((first, [])) => format!("{first} {item}"),
        Some((first, rest)) => format!("{first} ({}) {item}", rest.join(", ")),
        None => item.to_string(),
    };
    if let Some(note) = ingredient.get("note").and_then(Value::as_str) {
        if !note.is_empty() {
            line.push_str(&format!(", {note}"));
        }
    }
    line
}

fn parsed_ingredient(line: &str, section: Option<Value>) -> Result<Value, String> {
    let line = one_line(line)?;
    let parsed = parse_ingredient(&line).normalize_amounts();
    let mut ingredient =
        serde_json::to_value(parsed).map_err(|e| format!("Couldn't read that line: {e}"))?;
    ingredient["raw"] = Value::String(line);
    ingredient["section"] = section.unwrap_or(Value::Null);
    Ok(ingredient)
}

/// Whether a measurement is one item's size ("10 oz each"), as the clients'
/// ingredient formatting decides it.
fn is_per_item(measurement: &Value) -> bool {
    measurement
        .get("unit")
        .and_then(Value::as_str)
        .is_some_and(|unit| unit.trim().to_lowercase().ends_with(" each"))
}

/// An amount as recipes store them: up to 3 decimals, no trailing zeros.
pub(crate) fn format_amount(amount: f64) -> String {
    let text = format!("{amount:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    text.to_string()
}

fn one_line(text: &str) -> Result<String, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("The text is empty".to_string());
    }
    if text.contains('\n') {
        return Err("Give one line; add more lines one call at a time".to_string());
    }
    Ok(text.to_string())
}

fn string_arg(args: &Value, name: &str) -> Result<String, String> {
    args.get(name)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("{name} must be a string"))
}

fn number_arg(args: &Value, name: &str) -> Result<f64, String> {
    args.get(name)
        .and_then(Value::as_f64)
        .ok_or_else(|| format!("{name} must be a number"))
}

/// A 1-based index argument into a list of `count`, as a 0-based index.
fn index_arg(args: &Value, name: &str, count: usize) -> Result<usize, String> {
    let number = number_arg(args, name)?;
    if number.fract() != 0.0 || number < 1.0 || number > count as f64 {
        return Err(format!("{name} must be a whole number from 1 to {count}"));
    }
    Ok(number as usize - 1)
}

/// A position argument ("after N", 0 for first) into a list of `count`.
fn position_arg(args: &Value, name: &str, count: usize) -> Result<usize, String> {
    let number = number_arg(args, name)?;
    if number.fract() != 0.0 || number < 0.0 || number > count as f64 {
        return Err(format!("{name} must be a whole number from 0 to {count}"));
    }
    Ok(number as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft() -> RecipeDraft {
        RecipeDraft::new(json!({
            "title": "Soup",
            "ingredients": [
                {"item": "water", "measurements": [{"amount": "2", "unit": "cup"}], "note": null, "raw": "2 cups water", "section": null},
                {"item": "salt", "measurements": [{"amount": "1/2", "unit": "tsp"}], "note": "to taste", "raw": null, "section": null},
                {"item": "parsley", "measurements": [], "note": null, "raw": null, "section": "To serve"}
            ],
            "instructions": "Boil the water.\n\nAdd salt.\n\nServe.",
            "tags": ["dinner"],
            "servings": "4",
            "source_url": "https://example.com/soup"
        }))
        .unwrap()
    }

    async fn call(draft: &mut RecipeDraft, name: &str, args: Value) -> String {
        run_tool(draft, &NoUserData, name, &args.to_string()).await
    }

    #[tokio::test]
    async fn render_numbers_ingredients_by_section_and_steps() {
        let rendered = draft().render();
        assert!(rendered.contains(
            "  1. 2 cup water\n  2. 1/2 tsp salt, to taste\n  [To serve]\n  3. parsley\n"
        ));
        assert!(rendered.contains("Steps:\n  1. Boil the water.\n  2. Add salt.\n  3. Serve.\n"));
    }

    #[tokio::test]
    async fn scale_multiplies_parsed_amounts_only() {
        let mut draft = draft();
        call(&mut draft, "scale_recipe", json!({"factor": 1.5})).await;
        let recipe = draft.into_value();
        assert_eq!(recipe["ingredients"][0]["measurements"][0]["amount"], "3");
        assert_eq!(
            recipe["ingredients"][1]["measurements"][0]["amount"],
            "0.75"
        );
        // Nothing else changes.
        assert_eq!(recipe["servings"], "4");
        assert_eq!(recipe["ingredients"][0]["raw"], "2 cups water");
    }

    #[tokio::test]
    async fn scale_leaves_per_item_sizes_alone() {
        let mut draft = RecipeDraft::new(json!({
            "title": "Bars",
            "ingredients": [{
                "item": "marshmallows",
                "measurements": [{"amount": "2", "unit": "package"}, {"amount": "10", "unit": "oz each"}],
                "note": null, "raw": null, "section": null
            }],
            "instructions": "Melt."
        }))
        .unwrap();
        call(&mut draft, "scale_recipe", json!({"factor": 2})).await;
        let measurements = &draft.into_value()["ingredients"][0]["measurements"];
        assert_eq!(measurements[0]["amount"], "4");
        assert_eq!(measurements[1]["amount"], "10");
    }

    #[tokio::test]
    async fn set_ingredient_parses_the_line_and_keeps_its_section() {
        let mut draft = draft();
        let reply = call(
            &mut draft,
            "set_ingredient",
            json!({"index": 3, "line": "2 tbsp chopped parsley"}),
        )
        .await;
        assert!(reply.starts_with("Ingredient 3 is now"), "{reply}");
        let recipe = draft.into_value();
        let parsley = &recipe["ingredients"][2];
        assert_eq!(parsley["section"], "To serve");
        assert_eq!(parsley["measurements"][0]["unit"], "tbsp");
        assert_eq!(parsley["raw"], "2 tbsp chopped parsley");
        // The others are untouched.
        assert_eq!(recipe["ingredients"][0]["raw"], "2 cups water");
    }

    fn items(draft: RecipeDraft) -> Vec<(String, Value)> {
        draft.into_value()["ingredients"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| {
                (
                    i["item"].as_str().unwrap().to_string(),
                    i["section"].clone(),
                )
            })
            .collect()
    }

    #[tokio::test]
    async fn move_ingredient_counts_positions_before_the_move() {
        let mut draft = draft();
        call(
            &mut draft,
            "move_ingredient",
            json!({"index": 1, "after": 3}),
        )
        .await;
        assert_eq!(
            items(draft),
            [
                ("salt".to_string(), Value::Null),
                ("parsley".to_string(), json!("To serve")),
                ("water".to_string(), Value::Null),
            ]
        );

        let mut draft = super::tests::draft();
        call(
            &mut draft,
            "move_ingredient",
            json!({"index": 3, "after": 0}),
        )
        .await;
        assert_eq!(items(draft)[0].0, "parsley");
    }

    #[tokio::test]
    async fn set_section_names_or_clears_a_range() {
        let mut draft = draft();
        call(
            &mut draft,
            "set_section",
            json!({"from": 1, "to": 2, "section": "For the soup"}),
        )
        .await;
        call(
            &mut draft,
            "set_section",
            json!({"from": 3, "to": 3, "section": ""}),
        )
        .await;
        assert_eq!(
            items(draft),
            [
                ("water".to_string(), json!("For the soup")),
                ("salt".to_string(), json!("For the soup")),
                ("parsley".to_string(), Value::Null),
            ]
        );

        let mut draft = super::tests::draft();
        let reply = call(
            &mut draft,
            "set_section",
            json!({"from": 2, "to": 1, "section": "x"}),
        )
        .await;
        assert_eq!(reply, "Error: to must not be before from");
    }

    #[tokio::test]
    async fn step_edits_keep_the_instructions_spacing() {
        let mut draft = draft();
        call(
            &mut draft,
            "set_step",
            json!({"index": 2, "text": "Add a pinch of salt."}),
        )
        .await;
        call(
            &mut draft,
            "add_step",
            json!({"after": 3, "text": "Garnish."}),
        )
        .await;
        call(&mut draft, "remove_step", json!({"index": 1})).await;
        assert_eq!(
            draft.into_value()["instructions"],
            "Add a pinch of salt.\n\nServe.\n\nGarnish."
        );
    }

    #[tokio::test]
    async fn bad_arguments_are_reported_to_the_model() {
        let mut draft = draft();
        let reply = call(&mut draft, "remove_ingredient", json!({"index": 9})).await;
        assert_eq!(reply, "Error: index must be a whole number from 1 to 3");
        let reply = call(
            &mut draft,
            "set_field",
            json!({"field": "source_url", "value": "x"}),
        )
        .await;
        assert!(
            reply.starts_with("Error: source_url isn't a field"),
            "{reply}"
        );
        let reply = run_tool(&mut draft, &NoUserData, "set_step", "{not json").await;
        assert!(
            reply.starts_with("Error: the arguments aren't valid JSON"),
            "{reply}"
        );
    }

    #[tokio::test]
    async fn lookup_reports_catalog_weights_and_calories() {
        let mut draft = draft();
        let reply = call(
            &mut draft,
            "lookup_ingredient",
            json!({"name": "all-purpose flour"}),
        )
        .await;
        let found: Value = serde_json::from_str(&reply).expect(&reply);
        assert!(found["grams_per_cup"].as_f64().is_some(), "{reply}");
        assert!(found["kcal_per_100g"].as_f64().is_some(), "{reply}");
    }

    #[test]
    fn tool_schemas_are_objects() {
        for tool in edit_recipe_tools() {
            assert_eq!(tool.parameters["type"], "object", "{}", tool.name);
        }
    }
}
