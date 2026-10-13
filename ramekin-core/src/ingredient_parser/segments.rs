//! Splits the text after the measurement into segments at its top-level
//! separators: commas, "or", "plus"/"+", "/" and a spaced " - ". The item
//! and note passes in `parse_ingredient` classify these segments instead of
//! re-finding their boundaries with `find(" or ")` / `find(',')`, so a
//! separator inside brackets or parentheses, "and/or", or a "1,000" never
//! splits the line.

use super::tokens::{tokenize, Kind, Token};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Sep {
    Comma,
    Or,
    /// "plus" or "+".
    Plus,
    Slash,
    /// A dash with space on both sides ("apples - peeled").
    Dash,
}

/// A top-level separator, with the text around it trimmed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Split<'a> {
    pub sep: Sep,
    /// The "+" sign rather than the word "plus".
    pub sign: bool,
    /// A comma comes right before this separator (", plus more", ", or").
    pub follows_comma: bool,
    /// Everything before the separator.
    pub before: &'a str,
    /// Everything after the separator.
    pub after: &'a str,
    /// The separator and everything after it ("plus more for dusting").
    pub from_sep: &'a str,
    /// The segment after the separator, up to the next one.
    pub segment: &'a str,
}

/// A string split once at its top-level separators.
pub(super) struct Segments<'a> {
    head: &'a str,
    splits: Vec<Split<'a>>,
}

fn trimmed(s: &str, from: usize, to: usize) -> &str {
    s.get(from..to).unwrap_or("").trim()
}

impl<'a> Segments<'a> {
    pub fn new(s: &'a str) -> Self {
        let tokens = tokenize(s);
        let nested = nested_tokens(&tokens);
        let mut spans: Vec<(Sep, bool, usize, usize)> = Vec::new();
        for (i, token) in tokens.iter().enumerate() {
            if nested[i] {
                continue;
            }
            let prev = i.checked_sub(1).and_then(|p| tokens.get(p));
            if let Some((sep, sign)) = separator(token, prev, tokens.get(i + 1)) {
                spans.push((sep, sign, token.start, token.end));
            }
        }
        let head_end = spans.first().map_or(s.len(), |&(_, _, start, _)| start);
        let splits = spans
            .iter()
            .enumerate()
            .map(|(i, &(sep, sign, start, end))| {
                let next_start = spans.get(i + 1).map_or(s.len(), |&(_, _, start, _)| start);
                let follows_comma = i.checked_sub(1).and_then(|p| spans.get(p)).is_some_and(
                    |&(prev, _, _, prev_end)| {
                        prev == Sep::Comma && trimmed(s, prev_end, start).is_empty()
                    },
                );
                Split {
                    sep,
                    sign,
                    follows_comma,
                    before: trimmed(s, 0, start),
                    after: trimmed(s, end, s.len()),
                    from_sep: trimmed(s, start, s.len()),
                    segment: trimmed(s, end, next_start),
                }
            })
            .collect();
        Segments {
            head: trimmed(s, 0, head_end),
            splits,
        }
    }

    /// The text before the first separator.
    pub fn head(&self) -> &'a str {
        self.head
    }

    /// Every top-level separator, in order.
    pub fn splits(&self) -> &[Split<'a>] {
        &self.splits
    }

    /// The first top-level `sep`.
    pub fn first(&self, sep: Sep) -> Option<Split<'a>> {
        self.splits.iter().find(|s| s.sep == sep).copied()
    }

    /// The last top-level `sep`.
    pub fn last(&self, sep: Sep) -> Option<Split<'a>> {
        self.splits.iter().rfind(|s| s.sep == sep).copied()
    }

    /// The separator the string starts with ("or 3 cups", "/ 100g", "+ 2 tbsp").
    pub fn leading(&self) -> Option<Split<'a>> {
        self.splits.first().filter(|s| s.before.is_empty()).copied()
    }

    /// The text after `sep` when the string starts with it.
    pub fn leading_after(&self, sep: Sep) -> Option<&'a str> {
        self.leading().filter(|s| s.sep == sep).map(|s| s.after)
    }
}

/// Which tokens sit inside a closed pair of parentheses or brackets. An
/// unclosed "(" ("celery (about 8 ounces, chopped") nests nothing, so the
/// commas after it still split.
fn nested_tokens(tokens: &[Token]) -> Vec<bool> {
    let mut nested = vec![false; tokens.len()];
    let mut open: Vec<usize> = Vec::new();
    for (i, token) in tokens.iter().enumerate() {
        match token.text {
            "(" | "[" => open.push(i),
            ")" | "]" => {
                if let Some(start) = open.pop() {
                    nested[start..=i].iter_mut().for_each(|n| *n = true);
                }
            }
            _ => {}
        }
    }
    nested
}

/// The separator `token` stands for at the top level, if any, and whether it
/// is the "+" sign.
fn separator(token: &Token, prev: Option<&Token>, next: Option<&Token>) -> Option<(Sep, bool)> {
    // `prev` sits right against this token, or `next` right after it.
    let glued = |f: fn(&Token) -> bool| {
        prev.is_some_and(|p| f(p) && !token.space_before)
            || next.is_some_and(|n| f(n) && !n.space_before)
    };
    match token.kind {
        // "1,000": a thousands separator.
        Kind::Comma => {
            let digits_around = prev.is_some_and(|p| p.is_integer())
                && next.is_some_and(|n| n.is_integer() && !n.space_before);
            (!digits_around).then_some((Sep::Comma, false))
        }
        // "and/or", "either/or": the slash joins the words.
        Kind::Word if token.is_word("or") => {
            (!glued(|t| t.kind == Kind::Slash)).then_some((Sep::Or, false))
        }
        // "Vegetable Plus! Oil": a brand, not "plus".
        Kind::Word if token.is_word("plus") => {
            let ends_phrase = next.is_none_or(|n| n.space_before || n.kind == Kind::Comma);
            ends_phrase.then_some((Sep::Plus, false))
        }
        Kind::Plus => Some((Sep::Plus, true)),
        Kind::Slash => (!glued(|t| t.is_word("or"))).then_some((Sep::Slash, false)),
        Kind::Dash => {
            let spaced = token.space_before && next.is_some_and(|n| n.space_before);
            spaced.then_some((Sep::Dash, false))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seps(s: &str) -> Vec<(Sep, &str)> {
        Segments::new(s)
            .splits()
            .iter()
            .map(|split| (split.sep, split.segment))
            .collect()
    }

    #[test]
    fn splits_at_top_level_separators() {
        let segments = Segments::new("feta, drained or ricotta plus more");
        assert_eq!(segments.head(), "feta");
        assert_eq!(
            seps("feta, drained or ricotta plus more"),
            vec![
                (Sep::Comma, "drained"),
                (Sep::Or, "ricotta"),
                (Sep::Plus, "more"),
            ]
        );
        assert_eq!(
            seps("apples - peeled + more"),
            vec![(Sep::Dash, "peeled"), (Sep::Plus, "more")]
        );
    }

    #[test]
    fn leaves_nested_and_joined_separators_alone() {
        assert!(seps("spinach [see note, or not] and/or kale").is_empty());
        assert!(seps("1,000 grains rice").is_empty());
        assert!(seps("dark-brown sugar").is_empty());
        assert!(seps("either/or").is_empty());
        assert!(seps("Vegetable Plus! Oil").is_empty());
        assert_eq!(
            seps("celery (about 8 ounces, chopped"),
            vec![(Sep::Comma, "chopped")]
        );
    }

    #[test]
    fn split_text() {
        let segments = Segments::new("salt, plus more to taste");
        let plus = segments.first(Sep::Plus).unwrap();
        assert!(plus.follows_comma);
        assert!(!plus.sign);
        assert_eq!(plus.before, "salt,");
        assert_eq!(plus.after, "more to taste");
        assert_eq!(plus.from_sep, "plus more to taste");

        let segments = Segments::new("a, b, c");
        assert_eq!(segments.first(Sep::Comma).unwrap().after, "b, c");
        assert_eq!(segments.last(Sep::Comma).unwrap().before, "a, b");
        assert!(!segments.last(Sep::Comma).unwrap().follows_comma);
    }

    #[test]
    fn leading_separator() {
        let segments = Segments::new("or 3 cups pineapple, diced");
        assert_eq!(
            segments.leading_after(Sep::Or),
            Some("3 cups pineapple, diced")
        );
        assert_eq!(segments.leading_after(Sep::Comma), None);
        assert_eq!(
            Segments::new("/ 100g celery root").leading_after(Sep::Slash),
            Some("100g celery root")
        );
        assert!(Segments::new("+ 2 tbsp").leading().unwrap().sign);
        assert_eq!(Segments::new("salt or pepper").leading(), None);
    }
}
