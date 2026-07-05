//! Shared fraction definitions used by parsing and extraction heuristics.

const UNICODE_FRACTIONS: &[(char, &str)] = &[
    ('½', "1/2"),
    ('⅓', "1/3"),
    ('⅔', "2/3"),
    ('¼', "1/4"),
    ('¾', "3/4"),
    ('⅕', "1/5"),
    ('⅖', "2/5"),
    ('⅗', "3/5"),
    ('⅘', "4/5"),
    ('⅙', "1/6"),
    ('⅚', "5/6"),
    ('⅛', "1/8"),
    ('⅜', "3/8"),
    ('⅝', "5/8"),
    ('⅞', "7/8"),
];

pub(in crate::ingredient_parser) fn unicode_fraction_ascii(c: char) -> Option<&'static str> {
    UNICODE_FRACTIONS
        .iter()
        .find_map(|&(fraction, ascii)| (fraction == c).then_some(ascii))
}

pub(crate) fn unicode_fraction_regex_class() -> &'static str {
    "½⅓⅔¼¾⅕⅖⅗⅘⅙⅚⅛⅜⅝⅞"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regex_class_matches_unicode_fraction_table() {
        let table_chars: String = UNICODE_FRACTIONS
            .iter()
            .map(|&(fraction, _)| fraction)
            .collect();
        assert_eq!(unicode_fraction_regex_class(), table_chars);
    }
}
