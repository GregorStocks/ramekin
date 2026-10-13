//! The measurement grammar: reads an amount, unit and package size off the
//! front of a string from its tokens.
//!
//! ```text
//! measurement := [modifier] (amount | a|an before a sized unit)? ["+" fraction] [modifier]
//!                (unit_phrase | of [a|an|the] [size])? [each [of [a|an|the]]]
//! amount      := number ["."] [(["-"] to | or | "-") number]
//! number      := int fraction | int "-" fraction | int (and | &) fraction | int | decimal | fraction
//! unit_phrase := "x" unit_phrase                         // "2 x 400g cans"
//!              | number ["-"] mass ["."] container        // "15 ounce can", "14-oz. can"
//!              | size ["-"] inch piece                    // "3-inch piece", "3-4 inch chunk"
//!              | "-" (mass | inch) [descriptor]           // "1/2-pound", "1/2-inch piece"
//!              | size (container | sizeable_measure)      // "large can", "small handful"
//!              | unit
//! ```
//!
//! Whitespace around a dash never decides the parse on its own: "1- 2 cans"
//! and "1-2 cans" are the same range, and "1- 15 ounce can" is one sized can
//! because a mass and a container follow the second number.

use super::tokens::{tokenize, Kind, Token};
use super::*;

/// Sizes a measure can take ("small handful", "large bunch", "small head").
const SIZEABLE_MEASURES: &[&str] = &["handful", "bunch", "pinch", "head"];

/// Nouns for a piece cut to a size in inches ("3-inch piece", "3-4 inch chunk").
const INCH_PIECE_NOUNS: &[&str] = &[
    "piece", "pieces", "chunk", "chunks", "knob", "knobs", "hunk", "hunks",
];

/// A position in a tokenized string.
#[derive(Clone)]
pub(super) struct Cursor<'a> {
    s: &'a str,
    tokens: Vec<Token<'a>>,
    pos: usize,
}

impl<'a> Cursor<'a> {
    pub fn new(s: &'a str) -> Self {
        Cursor {
            s,
            tokens: tokenize(s),
            pos: 0,
        }
    }

    fn peek(&self, n: usize) -> Option<&Token<'a>> {
        self.tokens.get(self.pos + n)
    }

    fn offset(&self) -> usize {
        self.peek(0).map_or(self.s.len(), |t| t.start)
    }

    /// The text from the cursor on.
    pub fn rest(&self) -> &'a str {
        self.s.get(self.offset()..).unwrap_or("").trim()
    }

    /// The text from the cursor on, with runs of whitespace collapsed.
    pub fn collapsed_rest(&self) -> String {
        self.rest().split_whitespace().collect::<Vec<_>>().join(" ")
    }

    fn text_between(&self, from: usize, to: usize) -> &'a str {
        let start = self.tokens.get(from).map_or(self.s.len(), |t| t.start);
        let end = to
            .checked_sub(1)
            .and_then(|last| self.tokens.get(last))
            .map_or(start, |t| t.end);
        self.s.get(start..end).unwrap_or("")
    }

    fn take(&mut self) -> Option<Token<'a>> {
        let token = self.peek(0).copied()?;
        self.pos += 1;
        Some(token)
    }

    fn take_if(&mut self, f: impl Fn(&Token) -> bool) -> Option<Token<'a>> {
        self.peek(0).filter(|t| f(t))?;
        self.take()
    }

    fn take_word(&mut self, words: &[&str]) -> Option<Token<'a>> {
        self.take_if(|t| words.iter().any(|w| t.is_word(w)))
    }

    /// The longest of `phrases` the text starts with that ends on a token
    /// boundary, and the number of tokens it spans.
    fn phrase<'p>(&self, phrases: &[&'p str]) -> Option<(&'p str, usize)> {
        let start = self.offset();
        phrases
            .iter()
            .filter(|phrase| {
                self.s
                    .get(start..start + phrase.len())
                    .is_some_and(|text| text.eq_ignore_ascii_case(phrase))
            })
            .filter_map(|phrase| {
                let end = start + phrase.len();
                let count = self.tokens[self.pos..].iter().position(|t| t.end == end)? + 1;
                Some((*phrase, count))
            })
            .max_by_key(|(phrase, _)| phrase.len())
    }

    /// The character right after the token `n` ahead of the cursor.
    fn char_after(&self, n: usize) -> Option<char> {
        let end = self.peek(n)?.end;
        self.s.get(end..)?.chars().next()
    }

    /// Skip a leading "of", and "of a"/"of an"/"of the".
    fn skip_of(&mut self, with_article: bool) -> bool {
        if self.take_word(&["of"]).is_none() {
            return false;
        }
        if with_article {
            self.take_word(&["a", "an", "the"]);
        }
        true
    }
}

/// A measurement read off the front of a string.
#[derive(Debug, Clone)]
pub(super) struct Read {
    pub modifier: Option<String>,
    pub amount: Option<String>,
    pub unit: Option<String>,
    /// The text after the measurement, with runs of whitespace collapsed.
    pub rest: String,
}

impl Read {
    /// The unit with any measurement modifier in front ("heaping tbsp").
    pub fn modified_unit(&self) -> Option<String> {
        match (&self.modifier, &self.unit) {
            (Some(m), Some(u)) => Some(format!("{m} {u}")),
            (Some(m), None) => Some(m.clone()),
            (None, u) => u.clone(),
        }
    }
}

/// Read a number: "1 1/2", "1-1/2", "2 and 1/2", "3", ".5", "3/4".
fn number(c: &mut Cursor) -> Option<String> {
    let first = c.take_if(|t| t.kind == Kind::Number)?;
    if first.is_integer() {
        let mixed = match (c.peek(0), c.peek(1)) {
            (Some(fraction), _)
                if fraction.is_fraction() && fraction.space_before && !sizes_what_follows(c) =>
            {
                Some(1)
            }
            (Some(dash), Some(fraction))
                if dash.kind == Kind::Dash
                    && !dash.space_before
                    && fraction.is_fraction()
                    && !fraction.space_before =>
            {
                Some(2)
            }
            (Some(and), Some(fraction))
                if (and.is_word("and") || and.text == "&") && fraction.is_fraction() =>
            {
                Some(2)
            }
            _ => None,
        };
        if let Some(skip) = mixed {
            c.pos += skip - 1;
            let fraction = c.take()?;
            return Some(format!("{} {}", first.text, fraction.text));
        }
    }
    Some(first.text.to_string())
}

/// True when the number at the cursor is hyphenated to a size of what
/// follows rather than counting it: in "3 1/2-inch slices" and "1 1/4-ounce
/// packet" the fraction sizes the slices or packet, so it doesn't join the
/// number before it. "1 1/4-pound squash" is still 1 1/4 pounds.
fn sizes_what_follows(c: &Cursor) -> bool {
    let mut probe = c.clone();
    probe.pos += 1;
    // 4 ½"-thick chops": an inch mark.
    if probe
        .peek(0)
        .is_some_and(|t| matches!(t.text, "\"" | "”") && !t.space_before)
    {
        return true;
    }
    if probe
        .take_if(|t| t.kind == Kind::Dash && !t.space_before)
        .is_none()
    {
        return false;
    }
    let Some(base) = probe.take_if(|t| t.kind == Kind::Word && !t.space_before) else {
        return false;
    };
    if base.is_word("inch") || base.is_word("inches") {
        return true;
    }
    if !WEIGHT_UNITS_FOR_COMPOUND.contains(&base.lower().as_str()) {
        return false;
    }
    probe.take_if(|t| t.kind == Kind::Period && !t.space_before);
    probe.peek(0).is_some_and(|noun| {
        is_package_noun(noun.text) || HYPHENATED_DESCRIPTOR_NOUNS.contains(&noun.lower().as_str())
    })
}

/// True when the tokens after the cursor are a package size: a mass and a
/// container ("15 ounce can", "3-oz. package").
fn sizes_a_package(c: &Cursor) -> bool {
    let mut c = c.clone();
    c.take_if(|t| t.kind == Kind::Dash);
    let Some(mass) = c.take_if(|t| t.kind == Kind::Word) else {
        return false;
    };
    if !WEIGHT_UNITS_FOR_COMPOUND.contains(&mass.lower().as_str()) {
        return false;
    }
    c.take_if(|t| t.kind == Kind::Period);
    c.take_if(|t| t.kind == Kind::Word)
        .is_some_and(|word| is_package_noun(word.text))
}

/// Read an amount: a number or a range of two.
fn amount(c: &mut Cursor) -> Option<String> {
    let low = number(c)?;
    let before_connector = c.pos;
    // "1. Remove the rind": a list number's period.
    if c.peek(0)
        .is_some_and(|t| t.kind == Kind::Period && !t.space_before)
        && c.char_after(0).is_none_or(char::is_whitespace)
    {
        c.pos += 1;
        return Some(low);
    }
    // "1 1/4- to 1 1/2-pound": a dash before "to" repeats the unit to come.
    if c.peek(0).is_some_and(|t| t.kind == Kind::Dash) && c.peek(1).is_some_and(|t| t.is_word("to"))
    {
        c.pos += 1;
    }
    if let Some(connector) = c.take_word(&["to", "or"]) {
        // "1/4 to 1/2-inch pieces" sizes the pieces; it doesn't count them.
        if !sizes_what_follows(c) {
            if let Some(high) = number(c) {
                return Some(format!("{low} {} {high}", connector.lower()));
            }
        }
    }
    c.pos = before_connector;
    let Some(dash) = c.peek(0).filter(|t| t.kind == Kind::Dash).copied() else {
        return Some(low);
    };
    let Some(next) = c.peek(1).copied() else {
        return Some(low);
    };
    if next.kind != Kind::Number {
        return Some(low);
    }
    // "1 - 2" and "6-8" read as ranges; with a space on only one side ("1- 2",
    // "2 -3"), the dash may just be spacing: "1- 15 ounce can" is one can.
    let one_sided = dash.space_before != next.space_before;
    c.pos += 1;
    let after_dash = c.pos;
    let high = number(c);
    if one_sided && sizes_a_package(c) {
        c.pos = after_dash;
        return Some(low);
    }
    let Some(high) = high else {
        c.pos = before_connector;
        return Some(low);
    };
    let plausible = matches!(
        (parse_amount(&low), parse_amount(&high)),
        (Some(low), Some(high)) if high > low && high <= low * 4.0
    );
    if one_sided && !plausible {
        c.pos = before_connector;
        return Some(low);
    }
    Some(format!("{low}-{high}"))
}

/// Read a measurement modifier ("scant", "heaping", "lightly packed").
fn modifier(c: &mut Cursor) -> Option<String> {
    let (modifier, count) = c.phrase(MEASUREMENT_MODIFIERS)?;
    // "good quality chocolate" and "good-quality sausages" name how good the
    // food is, not a generous measure.
    if c.peek(count)
        .is_some_and(|t| t.is_word("quality") || !t.space_before)
    {
        return None;
    }
    c.pos += count;
    Some(modifier.to_string())
}

/// True when a unit can end where the token `n` ahead ends: at whitespace,
/// a period, a comma, the end, or a slash before another amount.
fn unit_boundary(c: &Cursor, n: usize) -> bool {
    match c.char_after(n) {
        None => true,
        Some(ch) if ch.is_whitespace() || ch == '.' || ch == ',' => true,
        Some('/') => {
            let mut after = c.clone();
            after.pos += n + 2;
            after.peek(0).is_some_and(|t| t.kind == Kind::Number)
        }
        Some(_) => false,
    }
}

/// Read a plain unit ("cups", "tbsp", "T.", "fl. oz", "large") and the
/// periods after it.
fn unit(c: &mut Cursor) -> Option<String> {
    // "T." is a tablespoon, "t." a teaspoon.
    if let (Some(letter), Some(period)) = (c.peek(0), c.peek(1)) {
        let unit = match letter.text {
            "T" => Some("tbsp"),
            "t" => Some("tsp"),
            _ => None,
        };
        if let Some(unit) = unit.filter(|_| {
            period.kind == Kind::Period
                && !period.space_before
                && c.char_after(1)
                    .is_none_or(|ch| ch.is_whitespace() || ch == ',')
        }) {
            c.pos += 2;
            return Some(unit.to_string());
        }
    }
    let (unit, count) = c.phrase(&UNITS_SORTED)?;
    if !unit_boundary(c, count - 1) {
        return None;
    }
    c.pos += count;
    while c.take_if(|t| t.kind == Kind::Period).is_some() {}
    Some(unit.to_string())
}

/// Read a package size: "15 ounce can", "14-oz. can" (and "14- ounce can"
/// as "14-ounce can").
fn package_size(c: &mut Cursor) -> Option<String> {
    let mut probe = c.clone();
    let size = probe.take_if(|t| t.kind == Kind::Number)?;
    if !is_amount_like(size.text) {
        return None;
    }
    let dash = probe.take_if(|t| t.kind == Kind::Dash);
    if dash.is_some_and(|d| d.space_before) {
        return None;
    }
    let mass = probe.take_if(|t| t.kind == Kind::Word)?;
    if dash.is_none() && !mass.space_before {
        // "400g can" is read as a metric amount, not a package size here.
        return None;
    }
    if !WEIGHT_UNITS_FOR_COMPOUND.contains(&mass.lower().as_str()) {
        return None;
    }
    let period = probe.take_if(|t| t.kind == Kind::Period && !t.space_before);
    let container = *probe
        .peek(0)
        .filter(|t| t.kind == Kind::Word && is_package_noun(t.text))?;
    if !unit_boundary(&probe, 0) {
        return None;
    }
    probe.pos += 1;
    *c = probe;
    Some(format!(
        "{}{}{}{} {}",
        size.text,
        if dash.is_some() { "-" } else { " " },
        mass.text,
        period.map_or("", |p| p.text),
        container.text
    ))
}

/// Read a size in inches before a piece noun: "3-inch piece", "3-4 inch
/// chunk", "1 1/2-inch knob".
fn inch_piece(c: &mut Cursor) -> Option<String> {
    let start = c.pos;
    let mut probe = c.clone();
    let whole = number(&mut probe)?;
    // "1 1/2-inch knob": the fraction completes the size.
    if whole.bytes().all(|b| b.is_ascii_digit()) {
        probe.take_if(|t| t.is_fraction() && t.space_before);
    }
    let is_range = probe.peek(0).is_some_and(|t| t.kind == Kind::Dash)
        && probe.peek(1).is_some_and(|t| t.kind == Kind::Number);
    if is_range {
        probe.pos += 1;
        number(&mut probe)?;
    }
    match probe.take_if(|t| t.kind == Kind::Dash) {
        Some(dash) if dash.space_before => return None,
        Some(_) => {}
        None if probe.peek(0).is_some_and(|t| !t.space_before) => return None,
        None => {}
    }
    probe.take_word(&["inch", "inches"])?;
    probe.take_word(INCH_PIECE_NOUNS)?;
    let unit = probe.text_between(start, probe.pos).to_string();
    probe.skip_of(false);
    *c = probe;
    Some(unit)
}

/// After an amount, read a dash and a mass or inch unit ("1/2-pound",
/// "14-ounce can"). A descriptor noun after it makes the whole size the unit
/// of one ("1 14-ounce can"); returns the replacement amount if so.
fn hyphenated_unit(c: &mut Cursor, amount_start: usize) -> Option<(Option<String>, String)> {
    let mut probe = c.clone();
    // "1- inch knob" and "14.5 -ounce": a space on either side of the dash is
    // a typo for "1-inch" and "14.5-ounce".
    probe.take_if(|t| t.kind == Kind::Dash)?;
    let base = probe.take_if(|t| t.kind == Kind::Word)?;
    let base_lower = base.lower();
    if !WEIGHT_UNITS_FOR_COMPOUND.contains(&base_lower.as_str())
        && !["inch", "inches"].contains(&base_lower.as_str())
    {
        return None;
    }
    let period = probe.take_if(|t| t.kind == Kind::Period && !t.space_before);
    let after_base = probe.pos;
    let result = match probe.take_word(HYPHENATED_DESCRIPTOR_NOUNS) {
        Some(noun) => (
            Some("1".to_string()),
            format!(
                "{}-{}{} {}",
                c.text_between(amount_start, c.pos),
                base.text,
                period.map_or("", |p| p.text),
                noun.text
            ),
        ),
        None => {
            probe.pos = after_base;
            (None, base.text.to_string())
        }
    };
    probe.skip_of(false);
    *c = probe;
    Some(result)
}

/// "small handful", "large can": a size before a measure or container sizes
/// it, not the food.
fn sized_unit(c: &mut Cursor, size: &str) -> Option<String> {
    if !is_size_unit(size) {
        return None;
    }
    let mut probe = c.clone();
    let measure = unit(&mut probe).or_else(|| {
        probe
            .take_if(|t| t.kind == Kind::Word && is_package_noun(t.text))
            .map(|t| t.text.to_string())
    })?;
    let measure = normalize_unit(&measure);
    if !SIZEABLE_MEASURES.contains(&measure.as_str()) && !is_package_noun(&measure) {
        return None;
    }
    probe.skip_of(true);
    // "2 large pinches (of saffron)": the food went to the note, so the
    // measure stays as the item rather than leave it empty.
    if probe.rest().is_empty() {
        return None;
    }
    *c = probe;
    Some(format!("{} {measure}", normalize_unit(size)))
}

/// Read the unit phrase after an amount.
fn unit_phrase(c: &mut Cursor, amount_start: usize) -> Option<(Option<String>, String)> {
    // "2 x 400g cans": a multiplier before the unit.
    if c.pos > amount_start {
        let marker = c
            .peek(0)
            .filter(|t| t.is_word("x") || t.text == "×")
            .is_some();
        if marker {
            let mut probe = c.clone();
            probe.pos += 1;
            if probe.peek(0).is_some() {
                if let Some(unit) = package_size(&mut probe)
                    .or_else(|| attached_package(&mut probe))
                    .or_else(|| unit(&mut probe))
                {
                    *c = probe;
                    return Some((None, unit));
                }
            }
        }
        if let Some(read) = hyphenated_unit(c, amount_start) {
            return Some(read);
        }
        // "2- cups": a dash typed between the amount and the unit.
        let stray_dash = c
            .peek(0)
            .is_some_and(|t| t.kind == Kind::Dash && !t.space_before)
            && c.peek(1).is_some_and(|t| {
                t.kind == Kind::Word
                    && t.space_before
                    && !["to", "or", "and"].contains(&t.lower().as_str())
            });
        if stray_dash {
            c.pos += 1;
        }
    }
    if let Some(unit) = package_size(c).or_else(|| inch_piece(c)) {
        c.skip_of(false);
        return Some((None, unit));
    }
    let base = unit(c)?;
    if let Some(sized) = sized_unit(c, &base) {
        return Some((None, sized));
    }
    c.skip_of(false);
    Some((None, base))
}

/// "400g cans" after a multiplier: a metric amount attached to a container.
fn attached_package(c: &mut Cursor) -> Option<String> {
    let mut probe = c.clone();
    let start = probe.pos;
    probe.take_if(|t| t.kind == Kind::Number)?;
    let metric = probe.take_if(|t| t.kind == Kind::Word && !t.space_before)?;
    if !ATTACHED_METRIC_UNITS.contains(&normalize_unit(metric.text).as_str()) {
        return None;
    }
    let container = probe.take_if(|t| t.kind == Kind::Word)?;
    if !CONTAINERS.contains(&container.lower().as_str()) {
        return None;
    }
    let unit = format!(
        "{} {}",
        probe.text_between(start, probe.pos - 1),
        container.text
    );
    *c = probe;
    Some(unit)
}

/// Read an amount alone off the front of `s`.
pub(super) fn read_amount(s: &str) -> (Option<String>, String) {
    let mut c = Cursor::new(s);
    let amount = amount(&mut c);
    (amount, c.collapsed_rest())
}

/// Read a plain unit alone off the front of `s`, and an "of" after it.
pub(super) fn read_unit(s: &str) -> (Option<String>, String) {
    let mut c = Cursor::new(s);
    match unit(&mut c) {
        Some(unit) => {
            c.skip_of(false);
            (Some(unit), c.collapsed_rest())
        }
        None => (None, s.trim().to_string()),
    }
}

/// Read a metric amount attached to its number off the front of `s`: "65g
/// granulated sugar", or "120g" before "/2.75 oz.".
pub(super) fn read_attached_metric(s: &str) -> Option<(Measurement, String)> {
    let mut c = Cursor::new(s);
    let amount = c.take_if(|t| t.kind == Kind::Number && !t.is_fraction())?;
    let unit = c.take_if(|t| t.kind == Kind::Word && !t.space_before)?;
    if !ATTACHED_METRIC_UNITS.contains(&unit.lower().as_str())
        || !c
            .s
            .get(unit.end..)
            .and_then(|after| after.chars().next())
            .is_none_or(|ch| ch.is_whitespace() || matches!(ch, ',' | '/' | '.'))
    {
        return None;
    }
    c.take_if(|t| t.kind == Kind::Period && !t.space_before);
    Some((
        Measurement {
            amount: Some(amount.text.to_string()),
            unit: Some(unit.text.to_string()),
        },
        c.rest().to_string(),
    ))
}

/// Read a hyphenated size off the front of `s`: "15-ounce" is (amount
/// "15", unit "ounce"), "14-ounce can of beans" is one "14-ounce can". Returns
/// the amount, the unit, and the text after them.
pub(super) fn read_hyphenated_size(s: &str) -> Option<(String, String, String)> {
    let mut c = Cursor::new(s);
    let amount = amount(&mut c)?;
    let (count, unit) = hyphenated_unit(&mut c, 0)?;
    Some((count.unwrap_or(amount), unit, c.collapsed_rest()))
}

/// Read a whole measurement off the front of `s`: modifiers, amount, unit
/// phrase, "each", and "of a" after an amount with no unit.
pub(super) fn read_measurement(s: &str) -> Read {
    let mut c = Cursor::new(s);
    let pre_amount_modifier = modifier(&mut c);
    let amount_start = c.pos;
    let mut amount = amount(&mut c);

    // "A 14-ounce can", "a 3-inch piece": the article counts one sized unit.
    if amount.is_none() && c.peek(0).is_some_and(|t| t.is_word("a") || t.is_word("an")) {
        let mut probe = c.clone();
        probe.pos += 1;
        if package_size(&mut probe.clone())
            .or_else(|| inch_piece(&mut probe.clone()))
            .is_some()
        {
            c = probe;
            amount = Some("1".to_string());
        }
    }

    // A "+" between the amount and the unit: "1 + 1/2 teaspoons" is a mixed
    // number, "1/4 + 1/8 teaspoon" sums two fractions, and in "3+ cups" the
    // "+" is stray.
    if let Some(low) = amount.clone() {
        if c.peek(0).is_some_and(|t| t.kind == Kind::Plus) {
            let mut probe = c.clone();
            probe.pos += 1;
            match number(&mut probe) {
                Some(next) if next.contains('/') => {
                    amount = Some(if low.bytes().all(|b| b.is_ascii_digit()) {
                        format!("{low} {next}")
                    } else {
                        format!("{low} plus {next}")
                    });
                    c = probe;
                }
                None if unit(&mut probe.clone()).is_some() => c.pos += 1,
                _ => {}
            }
        }
    }

    let pre_unit_modifier = modifier(&mut c);
    let mut unit = None;
    if amount.is_some() {
        if let Some((replacement, read)) = unit_phrase(&mut c, amount_start) {
            if replacement.is_some() {
                amount = replacement;
            }
            unit = Some(read);
        } else if c.skip_of(true) {
            // "1/2 of a small onion": the size after the article is the unit.
            let mut probe = c.clone();
            if let Some(size) = unit_phrase_size(&mut probe) {
                c = probe;
                unit = Some(size);
            }
        }
    } else if let Some(read) = unit_phrase(&mut c, amount_start).map(|(_, unit)| unit) {
        unit = Some(read);
    }

    // "1/2 tsp each salt and pepper": the measurement applies to each item.
    if c.take_word(&["each"]).is_some() {
        c.skip_of(true);
        unit = Some(match unit {
            Some(u) => format!("{u} each"),
            None => "each".to_string(),
        });
    }

    Read {
        modifier: pre_unit_modifier.or(pre_amount_modifier),
        amount,
        unit,
        rest: c.collapsed_rest(),
    }
}

/// A size word read as the unit, sizing a measure after it when one follows.
fn unit_phrase_size(c: &mut Cursor) -> Option<String> {
    let mut probe = c.clone();
    let size = unit(&mut probe).filter(|size| is_size_unit(size))?;
    let unit = sized_unit(&mut probe, &size).unwrap_or(size);
    *c = probe;
    Some(unit)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(s: &str) -> (Option<String>, Option<String>, String) {
        let read = read_measurement(s);
        let unit = read.modified_unit();
        (read.amount, unit, read.rest)
    }

    fn some(s: &str) -> Option<String> {
        Some(s.to_string())
    }

    #[test]
    fn amounts() {
        assert_eq!(read_amount("1 1/2 cups").0, some("1 1/2"));
        assert_eq!(read_amount("1-1/2 cups").0, some("1 1/2"));
        assert_eq!(read_amount("2 and 1/2 cups").0, some("2 1/2"));
        assert_eq!(read_amount("1 to 2 cups").0, some("1 to 2"));
        assert_eq!(read_amount("3 or 4 drops").0, some("3 or 4"));
        assert_eq!(read_amount("6-8 leaves").0, some("6-8"));
        assert_eq!(read_amount("1 - 2 cups").0, some("1-2"));
        assert_eq!(read_amount("2 1/2 - 4 1/2 cups").0, some("2 1/2-4 1/2"));
        assert_eq!(read_amount("1-1 1/2 cups").0, some("1-1 1/2"));
        assert_eq!(read_amount("1 1/2-2 cups").0, some("1 1/2-2"));
        assert_eq!(read_amount(".5 tsp").0, some(".5"));
        assert_eq!(read_amount("salt").0, None);
    }

    #[test]
    fn one_sided_dashes() {
        assert_eq!(
            read("2 -3 tsp salt"),
            (some("2-3"), some("tsp"), "salt".to_string())
        );
        assert_eq!(
            read("4- 5 cups flour"),
            (some("4-5"), some("cups"), "flour".to_string())
        );
        assert_eq!(
            read("1- 2 cans beans"),
            (some("1-2"), some("cans"), "beans".to_string())
        );
        // An implausible range is spacing, not a range.
        assert_eq!(read("1- 20 almonds").0, some("1"));
        assert_eq!(
            read("1- 15 ounce can tomatoes"),
            (some("1"), some("15 ounce can"), "tomatoes".to_string())
        );
        assert_eq!(
            read("1- 3 ounce package cream cheese"),
            (
                some("1"),
                some("3 ounce package"),
                "cream cheese".to_string()
            )
        );
    }

    #[test]
    fn a_size_before_a_container_sizes_the_container() {
        assert_eq!(
            read("1- 2 large cans tomatoes"),
            (some("1-2"), some("large can"), "tomatoes".to_string())
        );
        assert_eq!(
            read("2 small cans of tuna"),
            (some("2"), some("small can"), "tuna".to_string())
        );
        assert_eq!(
            read("1 small handful rosemary"),
            (some("1"), some("small handful"), "rosemary".to_string())
        );
        assert_eq!(
            read("2 large eggs"),
            (some("2"), some("large"), "eggs".to_string())
        );
    }

    #[test]
    fn package_and_piece_sizes() {
        assert_eq!(
            read("1 14-oz. can tomatoes"),
            (some("1"), some("14-oz. can"), "tomatoes".to_string())
        );
        assert_eq!(
            read("14-ounce can of chickpeas"),
            (some("1"), some("14-ounce can"), "chickpeas".to_string())
        );
        assert_eq!(
            read("1/2-inch piece of ginger"),
            (some("1"), some("1/2-inch piece"), "ginger".to_string())
        );
        assert_eq!(
            read("1/2-pound beef"),
            (some("1/2"), some("pound"), "beef".to_string())
        );
        assert_eq!(read("14.5 -ounce").1, some("ounce"));
        assert_eq!(
            read("2 3-4 inch chunks ginger"),
            (some("2"), some("3-4 inch chunks"), "ginger".to_string())
        );
        assert_eq!(
            read("2 x 400g cans beans"),
            (some("2"), some("400g cans"), "beans".to_string())
        );
    }

    #[test]
    fn modifiers_plus_and_each() {
        assert_eq!(
            read("scant 1 teaspoon salt"),
            (some("1"), some("scant teaspoon"), "salt".to_string())
        );
        assert_eq!(
            read("2 heaping tablespoons cocoa"),
            (some("2"), some("heaping tablespoons"), "cocoa".to_string())
        );
        assert_eq!(read("1 + 1/2 tsp salt").0, some("1 1/2"));
        assert_eq!(read("1/4 + 1/8 tsp salt").0, some("1/4 plus 1/8"));
        assert_eq!(
            read("3+ cups sugar"),
            (some("3"), some("cups"), "sugar".to_string())
        );
        assert_eq!(
            read("1/2 tsp each salt and pepper"),
            (some("1/2"), some("tsp each"), "salt and pepper".to_string())
        );
        assert_eq!(
            read("1/2 of a small onion"),
            (some("1/2"), some("small"), "onion".to_string())
        );
        assert_eq!(
            read("2 T. butter"),
            (some("2"), some("tbsp"), "butter".to_string())
        );
        assert_eq!(
            read("2 cloves of garlic"),
            (some("2"), some("cloves"), "garlic".to_string())
        );
    }
}
