//! Classifies the text left after the measurement ("feta, drained, or 1 cup
//! ricotta, plus more for topping") into the item and the parts that go to the
//! note: a "plus" addition, trailing prep and guidance, a mixing instruction or
//! source, an "or" alternative and a comma-separated conditional alternative.
//!
//! The text is split once at its top-level separators (`Segments`). Every
//! rule cuts a part off the end of what is still the item, so the item only
//! ever shrinks, and the rules run in the precedence order below against that
//! one split instead of each re-finding its boundaries on a shrinking string.

use super::measure;
use super::segments::{Segments, Sep, Split};
use super::tokens::{tokenize, Kind};
use super::*;

/// The item text after the measurement, split once. `end` marks where the
/// item still ends; everything after it has gone to the note.
struct ItemText<'a> {
    text: &'a str,
    segments: Segments<'a>,
    end: usize,
}

impl<'a> ItemText<'a> {
    fn new(text: &'a str) -> Self {
        ItemText {
            text,
            segments: Segments::new(text),
            end: text.len(),
        }
    }

    /// What is still the item.
    fn item(&self) -> &'a str {
        self.text.get(..self.end).unwrap_or("").trim()
    }

    /// The top-level separators still inside the item, in order.
    fn splits(&self) -> Vec<Split<'a>> {
        self.segments
            .splits()
            .iter()
            .filter(|split| split.end <= self.end)
            .copied()
            .collect()
    }

    /// The item text after `split`.
    fn after(&self, split: &Split) -> &'a str {
        self.text.get(split.end..self.end).unwrap_or("").trim()
    }

    /// The item text from `split` on, separator included ("plus more").
    fn with_sep(&self, split: &Split) -> &'a str {
        self.text.get(split.start..self.end).unwrap_or("").trim()
    }

    /// End the item at byte `offset`, dropping a comma left at its end.
    fn cut(&mut self, offset: usize) {
        let kept = self
            .text
            .get(..offset)
            .unwrap_or("")
            .trim_end()
            .trim_end_matches(',')
            .trim_end();
        self.end = kept.len();
    }

    /// End the item before `split`.
    fn cut_before(&mut self, split: &Split) {
        self.cut(split.start);
    }

    /// End the item after its first `words` whitespace-separated words, where
    /// a phrase rule ("sesame seeds to taste") split it between words.
    fn cut_after_words(&mut self, words: usize) {
        let item = self.text.get(..self.end).unwrap_or("");
        let word_end = words
            .checked_sub(1)
            .and_then(|last| item.split_whitespace().nth(last))
            .map_or(0, |word| {
                word.as_ptr() as usize - item.as_ptr() as usize + word.len()
            });
        self.cut(word_end);
    }
}

/// The note parts gathered while classifying.
#[derive(Default)]
struct Note(Option<String>);

impl Note {
    /// Add `part` before what the note already holds, joined by `sep`.
    fn prepend(&mut self, part: String, sep: &str) {
        self.0 = Some(match self.0.take() {
            Some(existing) => format!("{part}{sep}{existing}"),
            None => part,
        });
    }

    fn append(&mut self, part: String, sep: &str) {
        self.0 = Some(match self.0.take() {
            Some(existing) => format!("{existing}{sep}{part}"),
            None => part,
        });
    }
}

pub(super) struct Classified {
    pub item: String,
    pub note: Option<String>,
}

/// Classify `text`, the line after its measurement. A trailing "+ 1/8 tsp"
/// in the primary measurement's unit folds into `primary_amount`; parenthetical
/// notes deferred to an "or" or comma branch are taken from `deferred`.
pub(super) fn classify_item(
    text: &str,
    primary_amount: &mut Option<String>,
    primary_unit: Option<&str>,
    deferred: &mut Vec<DeferredParentheticalNote>,
) -> Classified {
    let mut line = ItemText::new(text);
    let mut note = Note::default();

    plus_note(&mut line, &mut note, primary_amount.is_some());
    plus_continuation(&mut line, primary_amount, primary_unit);
    if note.0.is_none() {
        plus_sign_guidance(&mut line, &mut note);
    }
    trailing_notes(&mut line, &mut note, deferred);
    phrase_notes(&mut line, &mut note);
    let shared_food = or_alternative(&mut line, &mut note, deferred);
    if shared_food.is_none() {
        conditional_alternative(&mut line, &mut note, deferred);
    }

    Classified {
        item: shared_food.unwrap_or_else(|| line.item().to_string()),
        note: note.0,
    }
}

/// "flour, plus more for dusting" always ends the item; "1 egg plus 1 yolk"
/// does unless the "plus" adds a measurement ("1 cup plus 2 tbsp", which the
/// measurement reader folds into the amount).
fn plus_note(line: &mut ItemText, note: &mut Note, measured_line: bool) {
    let words: Vec<Split> = line
        .splits()
        .into_iter()
        .filter(|s| s.sep == Sep::Plus && !s.sign)
        .collect();
    let plus = if let Some(plus) = words.iter().find(|s| s.follows_comma) {
        plus
    } else if let Some(plus) = words.first() {
        // Right after the measurement ("1 3/4 cups plus a pinch of sugar"),
        // a "plus" the measurement reader didn't fold into the amount takes
        // the rest of the line.
        let after = line.after(plus);
        if after.is_empty() || (plus.before.is_empty() && !measured_line) {
            return;
        }
        let (plus_prefix, _) = strip_plus_measurement_prefix(after);
        let measured = parse_plus_continuation_measurement(after).is_some();
        if measured
            && (plus.before.is_empty()
                || plus_prefix.is_none()
                || leading_measurement_consumes_all(plus.before))
        {
            return;
        }
        plus
    } else {
        return;
    };
    note.prepend(line.with_sep(plus).to_string(), ", ");
    line.cut_before(plus);
}

/// "3/4 tsp salt + 1/8 tsp": a trailing addition in the same unit folds into
/// the amount.
fn plus_continuation(
    line: &mut ItemText,
    primary_amount: &mut Option<String>,
    primary_unit: Option<&str>,
) {
    let (Some(amount), Some(unit)) = (primary_amount.as_ref(), primary_unit) else {
        return;
    };
    let Some(plus) = line
        .splits()
        .into_iter()
        .rfind(|s| s.sep == Sep::Plus && s.sign)
    else {
        return;
    };
    if plus.before.is_empty() {
        return;
    }
    if let Some((plus_amount, Some(plus_unit), rest)) =
        parse_range_continuation_measurement(line.after(&plus))
    {
        if rest.trim().is_empty() && units_share_base(unit, &plus_unit) {
            *primary_amount = Some(format!("{amount} plus {plus_amount}"));
            line.cut_before(&plus);
        }
    }
}

/// "sugar + more for sprinkling": the "+" spelling of ", plus more".
fn plus_sign_guidance(line: &mut ItemText, note: &mut Note) {
    let Some(plus) = line
        .splits()
        .into_iter()
        .find(|s| s.sep == Sep::Plus && s.sign)
    else {
        return;
    };
    let after = line.after(&plus);
    if plus.before.is_empty() || after.is_empty() {
        return;
    }
    let plus_note = format!("plus {after}");
    if is_trailing_guidance_note(&plus_note) {
        note.prepend(plus_note, ", ");
        line.cut_before(&plus);
    }
}

/// Trailing comma parts that qualify the item ("feta, drained, crumbled",
/// "oil, for frying", "salt, divided"), right to left, stopping at the first
/// part that could name the item itself; never leaves only prep words behind.
/// Then a dash that sets off the first prep step ("apples - peeled").
fn trailing_notes(
    line: &mut ItemText,
    note: &mut Note,
    deferred: &mut Vec<DeferredParentheticalNote>,
) {
    // Each peeled part with where the item ended before peeling it.
    let mut peeled: Vec<(String, usize)> = Vec::new();
    while let Some(comma) = line.splits().into_iter().rfind(|s| s.sep == Sep::Comma) {
        let potential_note = line.after(&comma);
        let potential_item = comma.before;
        let guidance = is_trailing_guidance_note(potential_note);
        let lower_note = potential_note.to_lowercase();
        // "and sliced scallions" continues a list of foods; it is never a note.
        // A trailing "or ..." alternative still goes to the note.
        // "chopped mix-ins, like cooked meats": after a category, the
        // examples are the foods.
        let examples_of_category = (lower_note.starts_with("like ")
            || lower_note.starts_with("such as "))
            && item::has_generic_head(potential_item);
        // "and thinly sliced", "and cut into strips" finish a prep chain.
        let continues_list = lower_note.strip_prefix("and ").is_some_and(|rest| {
            !is_strict_trailing_prep_note(potential_note) && !item::is_prep_phrase(rest)
        }) || examples_of_category;
        // Only the last part may be a prep phrase that names a food
        // ("crumbled queso fresco"); every part before it must be a pure
        // note, or a list of foods ending in "for topping" would lose all
        // but its first line.
        let is_note = if peeled.is_empty() {
            is_trailing_prep_note(potential_note)
                || guidance
                || is_trailing_qualifier(potential_note)
        } else {
            is_strict_trailing_prep_note(potential_note)
                || item::is_prep_phrase(potential_note)
                || guidance
                || is_trailing_qualifier(potential_note)
        };
        let lower_item = potential_item.to_lowercase();
        // "a dish, or frankly, any old dish": never leave a dangling "or".
        let dangling = lower_item.ends_with(" or") || lower_item.ends_with(" and");
        if potential_note.is_empty()
            || potential_item.is_empty()
            || dangling
            || is_only_prep_words(potential_item)
            || (continues_list && !guidance)
            || !is_note
        {
            break;
        }
        let mut extracted_note = potential_note.to_string();
        if peeled.is_empty() && guidance {
            if let Some(branch_note) = take_last_or_parenthetical_note(deferred) {
                extracted_note = format!("{} ({})", extracted_note, branch_note);
            }
        }
        peeled.push((extracted_note, line.end));
        line.cut_before(&comma);
    }
    // "romaine, rinsed, patted dry, and chopped" where only "and chopped"
    // peeled: the chain's start wasn't recognized, so the "and" part goes
    // back to the line. Parts after it ("..., the remainder sliced thin")
    // stay peeled.
    if peeled.last().is_some_and(|(first, _)| {
        first.to_lowercase().starts_with("and ") && !is_trailing_guidance_note(first)
    }) {
        if let Some((_, end)) = peeled.pop() {
            line.end = end;
        }
    }
    // "apples - peeled, cored, and sliced": a dash sets off the first
    // prep step.
    if let Some(dash) = line.splits().into_iter().find(|s| s.sep == Sep::Dash) {
        let after = line.after(&dash);
        if !dash.before.is_empty() && is_strict_trailing_prep_note(after) {
            peeled.push((after.to_string(), line.end));
            line.cut_before(&dash);
        }
    }
    if !peeled.is_empty() {
        let parts: Vec<String> = peeled.into_iter().rev().map(|(part, _)| part).collect();
        note.prepend(parts.join(", "), ", ");
    }
}

/// Notes that end the item between words rather than at a separator.
fn phrase_notes(line: &mut ItemText, note: &mut Note) {
    // Trailing guidance without a comma ("sesame seeds to taste", "oil for
    // frying", "neutral oil such as canola", "spinach [see Note]").
    if let Some((item_part, trailing)) = split_trailing_phrase_note(line.item()) {
        note.prepend(trailing, ", ");
        line.cut_after_words(item_part.split_whitespace().count());
    }

    // A mixing instruction ("cornstarch mixed with 1 tablespoon water") or
    // the item's source ("tomato puree from a 28-ounce can"), along with
    // anything after it. A colon left in the item marks a labeled list ("To
    // flavor: 2 cups juice, ..."), which stays whole. Inside an "or" branch
    // ("stock or 2 teaspoons base dissolved in 2 cups water") the instruction
    // belongs to the alternative, which goes to the note whole.
    let text = line.item();
    if text.contains(':') {
        return;
    }
    let mixing_or_source = split_mixing_note(text)
        .or_else(|| split_container_source_note(text))
        .filter(|(item_part, _)| !item_part.to_lowercase().contains(" or "));
    if let Some((item_part, trailing)) = mixing_or_source {
        note.prepend(trailing, ", ");
        line.cut_after_words(item_part.split_whitespace().count());
    }
}

/// "dried Italian seasoning or a combination of 1/4 teaspoon dried oregano
/// ...": when the text after the first "or" measures something, the item is
/// what comes before it and the alternative goes to the note. Other "or"s
/// ("chicken or vegetable stock") name the food. Returns the item when a brand
/// before the "or" borrows the alternative's food.
fn or_alternative(
    line: &mut ItemText,
    note: &mut Note,
    deferred: &mut Vec<DeferredParentheticalNote>,
) -> Option<String> {
    let or = line.splits().into_iter().find(|s| s.sep == Sep::Or)?;
    let (before_or, after_or) = (or.before, line.after(&or));
    if before_or.is_empty() || after_or.is_empty() {
        return None;
    }

    // The alternative measures something before any comma after it ("a
    // combination of 1/4 teaspoon dried oregano, ..."), unless the text before
    // "or" is itself an amount or a size the alternative replaces part of
    // ("1/2 cup or 113g butter", "A 250-gram or 8.8-ounce package noodles").
    let alternative_end = line
        .splits()
        .into_iter()
        .find(|s| s.start > or.start && s.sep == Sep::Comma)
        .map_or(line.end, |comma| comma.start);
    let alternative = line.text.get(or.end..alternative_end).unwrap_or("");
    let measured = measures_something(alternative) && !is_amount_or_size(before_or);
    // "carrot or 2 slim ones", "garlic or 1 shallot": a bare count
    // alternative, as long as the text before "or" names the item rather than
    // a size or variety of the food after it ("thick or 2 slimmer leeks",
    // "Turkish or 1/2 California bay leaf").
    if !measured && !is_count_alternative_after_item(before_or, after_or) {
        return None;
    }

    let shared_food = brand_shared_food(before_or, after_or);
    let alternative = match take_last_or_parenthetical_note(deferred) {
        Some(alternative_note) => format!("or {} ({})", after_or, alternative_note),
        None => format!("or {}", after_or),
    };
    // A parenthetical taken out of the alternative leaves "bread , crusts".
    let alternative = alternative.replace(" ,", ",");
    note.prepend(alternative, "; ");
    line.cut_before(&or);
    shared_food
}

/// Whether `s` holds an amount with a unit ("a combination of 1/4 teaspoon
/// dried oregano"), outside any parentheses ("(I used 5 cups)"). A size alone
/// ("1/2 large California bay leaf") doesn't count.
fn measures_something(s: &str) -> bool {
    let mut outside = String::new();
    let mut depth: i32 = 0;
    for c in s.chars() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            _ if depth == 0 => outside.push(c),
            _ => {}
        }
    }
    tokenize(&outside)
        .iter()
        .filter(|token| token.kind == Kind::Number)
        .any(|token| {
            let read = measure::read_measurement(outside.get(token.start..).unwrap_or(""));
            read.amount.is_some() && read.unit.is_some_and(|unit| measures_amount(&unit))
        })
}

/// "1/2 cup", "A 250-gram", "medium-sized", "spare ribs about 2.2 pounds":
/// text that ends in an amount or a size rather than naming a food.
fn is_amount_or_size(s: &str) -> bool {
    let words: Vec<&str> = s.split_whitespace().collect();
    let ends_in_measurement = (1..=words.len().min(3)).any(|n| {
        let read = measure::read_measurement(&words[words.len() - n..].join(" "));
        read.amount.is_some() && read.unit.is_some() && read.rest.is_empty()
    });
    if ends_in_measurement || measure::read_measurement(s).rest.is_empty() {
        return true;
    }
    let last = words.last().copied().unwrap_or("");
    let last = last.strip_suffix("-sized").unwrap_or(last);
    // A bare number or a hyphenated size ("250-gram"), but not a name with a
    // digit in it ("7up").
    let number_len = last
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '/'))
        .unwrap_or(last.len());
    let numeric_size = number_len > 0
        && last
            .get(number_len..)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('-'));
    numeric_size || is_size_unit(last)
}

/// A unit that measures how much food there is, rather than its size
/// ("large") or the size of its pieces ("cut into 1-inch cubes").
fn measures_amount(unit: &str) -> bool {
    !is_size_unit(unit)
        && !unit
            .split(|c: char| c.is_whitespace() || c == '-')
            .any(|word| ["inch", "inches", "cm"].contains(&word.to_lowercase().as_str()))
}

/// A brand alone before "or" ("Diamond Crystal or 1 1/4 tsp. Morton kosher
/// salt") borrows the food from the alternative: "Diamond Crystal kosher
/// salt". A brand is a multi-word capitalized name ("Diamond Crystal", "Grand
/// Marnier"); a single capitalized word ("Tajín") is its own food, and a Title
/// Case food ("Chicken Stock or 2 cups water", "Better Than Bouillon or ...")
/// ends in a food noun and keeps its own name.
fn brand_shared_food(before_or: &str, after_or: &str) -> Option<String> {
    let brand_words: Vec<&str> = before_or.split_whitespace().collect();
    let brand_only = brand_words.len() >= 2
        && brand_words
            .iter()
            .all(|word| word.chars().next().is_some_and(char::is_uppercase))
        && !brand_words
            .last()
            .is_some_and(|last| TITLE_CASE_FOOD_NOUNS.contains(&last.to_lowercase().as_str()));
    if !brand_only {
        return None;
    }
    let alternative_item = parse_ingredient(after_or).item;
    let food: Vec<&str> = alternative_item
        .split_whitespace()
        .skip_while(|word| word.chars().next().is_some_and(char::is_uppercase))
        // "or another orange liqueur": the determiner isn't the food.
        .skip_while(|word| {
            ["another", "other", "a", "an", "any", "some", "your"]
                .contains(&word.to_lowercase().as_str())
        })
        .collect();
    // "King Arthur Rolled Oats or 1 cup rolled oats" already names the food;
    // only a name without it borrows the alternative's.
    let names_food = food.last().is_some_and(|last| {
        before_or
            .to_lowercase()
            .split_whitespace()
            .any(|word| word == last.to_lowercase())
    });
    (!food.is_empty() && !names_food).then(|| format!("{} {}", before_or, food.join(" ")))
}

/// "vegetable broth, 1 1/2 cups vegetable broth (for cooked chickpeas)": a
/// measured repeat of the item after the first comma is an alternative under
/// a condition.
fn conditional_alternative(
    line: &mut ItemText,
    note: &mut Note,
    deferred: &mut Vec<DeferredParentheticalNote>,
) {
    let Some(comma) = line.splits().into_iter().find(|s| s.sep == Sep::Comma) else {
        return;
    };
    let (before_comma, after_comma) = (comma.before, line.after(&comma));
    if before_comma.is_empty() || after_comma.is_empty() {
        return;
    }
    let read = measure::read_measurement(after_comma);
    let (Some(amount), Some(unit)) = (&read.amount, read.modified_unit()) else {
        return;
    };
    let after_unit = read.rest.as_str();
    if !after_unit
        .to_lowercase()
        .starts_with(&before_comma.to_lowercase())
    {
        return;
    }
    let conditional_part = after_unit
        .get(before_comma.len()..)
        .unwrap_or("")
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .trim();
    let condition = if conditional_part.is_empty() {
        take_last_comma_parenthetical_note(deferred)
    } else {
        Some(conditional_part.to_string())
    };
    let fragment = match condition {
        Some(condition) => format!("or {} {} {}", amount, normalize_unit(&unit), condition),
        None => format!("or {} {}", amount, normalize_unit(&unit)),
    };
    note.append(fragment, "; ");
    line.cut_before(&comma);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(raw: &str) -> (String, Option<String>) {
        let parsed = parse_ingredient(raw);
        (parsed.item, parsed.note)
    }

    fn note(s: &str) -> Option<String> {
        Some(s.to_string())
    }

    #[test]
    fn plus_after_a_compound_amount_goes_to_the_note() {
        assert_eq!(
            parsed("½ cup plus 2 tbsp all-purpose flour plus extra for dusting"),
            ("all-purpose flour".into(), note("plus extra for dusting"))
        );
        assert_eq!(
            parsed("1 cup flour, plus more for dusting"),
            ("flour".into(), note("plus more for dusting"))
        );
    }

    #[test]
    fn notes_keep_the_order_of_the_text() {
        assert_eq!(
            parsed("1 cup feta, drained, crumbled, plus more for topping"),
            (
                "feta".into(),
                note("drained, crumbled, plus more for topping")
            )
        );
    }

    #[test]
    fn or_alternative_needs_an_amount_with_a_unit() {
        assert_eq!(
            parsed("1 8-ounce block cream cheese or 1 8-ounce tub non-dairy cream cheese, at room temperature"),
            (
                "8-ounce block cream cheese".into(),
                note("or 1 8-ounce tub non-dairy cream cheese; at room temperature")
            )
        );
        // A piece size isn't an amount.
        assert_eq!(
            parsed("1/2 pound paneer or extra firm tofu cut into 1-inch cubes and patted very dry")
                .0,
            "paneer or extra firm tofu cut into 1-inch cubes and patted very dry"
        );
    }

    #[test]
    fn or_after_an_amount_or_size_stays_in_the_item() {
        // The unit list this replaced matched "cupcakes".
        assert!(!measures_something("3 cupcakes"));
        assert!(!measures_something("1-inch segment of vanilla bean"));
        assert!(measures_something(
            "a combination of 1/4 teaspoon dried oregano"
        ));
        assert!(is_amount_or_size("1/2 cup"));
        assert!(is_amount_or_size("A 250-gram"));
        assert!(is_amount_or_size("medium-sized"));
        assert!(is_amount_or_size("pork belly spare ribs about 2.2 pounds"));
        assert!(!is_amount_or_size("garlic cloves"));
        assert!(!is_amount_or_size("7up"));
        assert_eq!(
            parsed("1 cup 7up or 1 cup lemon-lime soda"),
            ("7up".into(), note("or 1 cup lemon-lime soda"))
        );
    }
}
