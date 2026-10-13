//! Tokenizer for ingredient lines: splits a line once into numbers, words and
//! punctuation, each with its byte span, so the measurement grammar in
//! `measure.rs` reads tokens instead of rewriting the string.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    /// An integer, decimal (".5", "1.25") or fraction ("1/2").
    Number,
    /// A run of letters, with inner apostrophes ("baker's").
    Word,
    Dash,
    Slash,
    Plus,
    Comma,
    Period,
    Open,
    Close,
    /// Any other single character ("&", "×", "%").
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Token<'a> {
    pub kind: Kind,
    pub text: &'a str,
    pub start: usize,
    pub end: usize,
    /// Whitespace separates this token from the one before it.
    pub space_before: bool,
}

impl Token<'_> {
    pub fn is_word(&self, word: &str) -> bool {
        self.kind == Kind::Word && self.text.eq_ignore_ascii_case(word)
    }

    pub fn lower(&self) -> String {
        self.text.to_lowercase()
    }

    /// An integer with no decimal point or fraction.
    pub fn is_integer(&self) -> bool {
        self.kind == Kind::Number && self.text.bytes().all(|b| b.is_ascii_digit())
    }

    pub fn is_fraction(&self) -> bool {
        self.kind == Kind::Number && self.text.contains('/')
    }
}

/// Split `s` into tokens. Digits and letters always split ("400g" is 400, g;
/// "1finely" is 1, finely), so attached units need no separate pass.
pub(super) fn tokenize(s: &str) -> Vec<Token<'_>> {
    let bytes = s.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0;
    let mut space_before = false;
    while i < s.len() {
        let Some(c) = s.get(i..).and_then(|rest| rest.chars().next()) else {
            break;
        };
        if c.is_whitespace() {
            space_before = true;
            i += c.len_utf8();
            continue;
        }
        let start = i;
        let starts_decimal =
            c == '.' && bytes.get(i + 1).is_some_and(|b| b.is_ascii_digit());
        let kind = if c.is_ascii_digit() || starts_decimal {
            i = number_end(bytes, i);
            Kind::Number
        } else if c.is_alphabetic() {
            i = word_end(s, i);
            Kind::Word
        } else {
            i += c.len_utf8();
            match c {
                '-' => Kind::Dash,
                '/' => Kind::Slash,
                '+' => Kind::Plus,
                ',' => Kind::Comma,
                '.' => Kind::Period,
                '(' => Kind::Open,
                ')' => Kind::Close,
                _ => Kind::Other,
            }
        };
        tokens.push(Token {
            kind,
            text: s.get(start..i).unwrap_or(""),
            start,
            end: i,
            space_before,
        });
        space_before = false;
    }
    tokens
}

/// End of the number starting at `i`: digits, one decimal part, and a
/// "/digits" denominator when the number is an integer.
fn number_end(bytes: &[u8], mut i: usize) -> usize {
    let digits = |mut i: usize| {
        while bytes.get(i).is_some_and(|b| b.is_ascii_digit()) {
            i += 1;
        }
        i
    };
    i = digits(i);
    let mut is_integer = true;
    if bytes.get(i) == Some(&b'.') && bytes.get(i + 1).is_some_and(|b| b.is_ascii_digit()) {
        i = digits(i + 1);
        is_integer = false;
    }
    if is_integer
        && bytes.get(i) == Some(&b'/')
        && bytes.get(i + 1).is_some_and(|b| b.is_ascii_digit())
    {
        i = digits(i + 1);
    }
    i
}

/// End of the word starting at `i`: letters, with an apostrophe kept only
/// between letters.
fn word_end(s: &str, mut i: usize) -> usize {
    let mut chars = s.get(i..).unwrap_or("").chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_alphabetic() {
            i += c.len_utf8();
        } else if matches!(c, '\'' | '’') && chars.peek().is_some_and(|n| n.is_alphabetic()) {
            i += c.len_utf8();
        } else {
            break;
        }
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(s: &str) -> Vec<(Kind, &str)> {
        tokenize(s).into_iter().map(|t| (t.kind, t.text)).collect()
    }

    #[test]
    fn numbers_fractions_and_attached_units() {
        assert_eq!(
            kinds("1 1/2 cups"),
            vec![
                (Kind::Number, "1"),
                (Kind::Number, "1/2"),
                (Kind::Word, "cups")
            ]
        );
        assert_eq!(
            kinds("400g/14oz"),
            vec![
                (Kind::Number, "400"),
                (Kind::Word, "g"),
                (Kind::Slash, "/"),
                (Kind::Number, "14"),
                (Kind::Word, "oz"),
            ]
        );
        assert_eq!(
            kinds(".5 tsp"),
            vec![(Kind::Number, ".5"), (Kind::Word, "tsp")]
        );
        assert_eq!(
            kinds("2.5/3"),
            vec![(Kind::Number, "2.5"), (Kind::Slash, "/"), (Kind::Number, "3")]
        );
    }

    #[test]
    fn dashes_and_spacing() {
        let tokens = tokenize("1- 2 cans");
        assert_eq!(tokens[1].kind, Kind::Dash);
        assert!(!tokens[1].space_before);
        assert!(tokens[2].space_before);
        assert_eq!(
            kinds("14-oz. can"),
            vec![
                (Kind::Number, "14"),
                (Kind::Dash, "-"),
                (Kind::Word, "oz"),
                (Kind::Period, "."),
                (Kind::Word, "can"),
            ]
        );
    }

    #[test]
    fn words_keep_inner_apostrophes() {
        assert_eq!(
            kinds("baker's 'chocolate'"),
            vec![
                (Kind::Word, "baker's"),
                (Kind::Other, "'"),
                (Kind::Word, "chocolate"),
                (Kind::Other, "'"),
            ]
        );
    }
}
