//! Hierarchical tag helpers.
//!
//! Tags use a `namespace:value` string convention layered over the flat
//! `user_tags.name` column. A tag is hierarchical iff it contains exactly
//! one colon and both sides are non-empty. Multi-colon names are always
//! "uncategorized" — we never split them. Storage always uses the raw
//! `name`; `format_tag` is a UI convenience and must not round-trip
//! through `parse_tag` for persistence.

use std::sync::LazyLock;

use regex::Regex;

/// Namespaces shown in the UI even when the user has no tags in them yet.
pub const SEEDED_NAMESPACES: &[&str] = &[
    "ingredient",
    "course",
    "cuisine",
    "diet",
    "method",
    "season",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedTag {
    pub namespace: Option<String>,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagNameError {
    Empty,
    MultipleColons,
    EmptyNamespace,
    EmptyValue,
    InvalidNamespace,
    WhitespaceAroundColon,
}

impl TagNameError {
    pub fn message(&self) -> &'static str {
        match self {
            TagNameError::Empty => "Tag name cannot be empty",
            TagNameError::MultipleColons => {
                "Tag name may contain at most one colon (namespace:value)"
            }
            TagNameError::EmptyNamespace => "Namespace cannot be empty",
            TagNameError::EmptyValue => "Tag value cannot be empty",
            TagNameError::InvalidNamespace => {
                "Namespace must be lowercase letters, digits, hyphen, or underscore, starting with a letter"
            }
            TagNameError::WhitespaceAroundColon => {
                "Whitespace is not allowed adjacent to ':'"
            }
        }
    }
}

static NAMESPACE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[a-z][a-z0-9_-]*$").unwrap());

/// Parse a tag name into its optional namespace and value. Never returns
/// an error; inputs that don't match the `namespace:value` shape are
/// returned as a value with `namespace = None`.
pub fn parse_tag(name: &str) -> ParsedTag {
    let trimmed = name.trim();
    let colon_count = trimmed.matches(':').count();
    if colon_count != 1 {
        return ParsedTag {
            namespace: None,
            value: trimmed.to_string(),
        };
    }
    let (ns, value) = trimmed.split_once(':').unwrap();
    let ns = ns.trim();
    let value = value.trim();
    if ns.is_empty() || value.is_empty() {
        return ParsedTag {
            namespace: None,
            value: trimmed.to_string(),
        };
    }
    ParsedTag {
        namespace: Some(ns.to_string()),
        value: value.to_string(),
    }
}

/// Construct a tag name from an optional namespace and a value. Whitespace
/// is trimmed from both sides. Caller is responsible for having validated
/// the inputs — this is a formatter, not a validator.
pub fn format_tag(namespace: Option<&str>, value: &str) -> String {
    let value = value.trim();
    match namespace.map(str::trim).filter(|s| !s.is_empty()) {
        Some(ns) => format!("{ns}:{value}"),
        None => value.to_string(),
    }
}

/// Validate a tag name under the hierarchical tag rules.
pub fn validate_tag_name(name: &str) -> Result<(), TagNameError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(TagNameError::Empty);
    }
    let colon_count = trimmed.matches(':').count();
    if colon_count == 0 {
        return Ok(());
    }
    if colon_count > 1 {
        return Err(TagNameError::MultipleColons);
    }
    let (ns, value) = trimmed.split_once(':').unwrap();
    if ns.is_empty() {
        return Err(TagNameError::EmptyNamespace);
    }
    if value.is_empty() {
        return Err(TagNameError::EmptyValue);
    }
    if ns != ns.trim_end() || value != value.trim_start() {
        return Err(TagNameError::WhitespaceAroundColon);
    }
    if !NAMESPACE_RE.is_match(ns) {
        return Err(TagNameError::InvalidNamespace);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct TagHierarchyVectors {
        seeded_namespaces: Vec<String>,
        parse_cases: Vec<ParseCase>,
        format_cases: Vec<FormatCase>,
        normalize_namespace_cases: Vec<NormalizeNamespaceCase>,
        group_cases: Vec<GroupCase>,
        known_namespaces_cases: Vec<KnownNamespacesCase>,
    }

    #[derive(Deserialize)]
    struct ParseCase {
        name: String,
        input: String,
        namespace: Option<String>,
        value: String,
    }

    #[derive(Deserialize)]
    struct FormatCase {
        name: String,
        namespace: Option<String>,
        value: String,
        expected: String,
    }

    #[derive(Deserialize)]
    struct NormalizeNamespaceCase {
        name: String,
        input: String,
        expected: Option<String>,
    }

    #[derive(Deserialize)]
    struct GroupCase {
        name: String,
        names: Vec<String>,
        expected: Vec<ExpectedGroup>,
    }

    #[derive(Debug, Deserialize, PartialEq, Eq)]
    struct ExpectedGroup {
        namespace: Option<String>,
        names: Vec<String>,
    }

    #[derive(Deserialize)]
    struct KnownNamespacesCase {
        name: String,
        names: Vec<String>,
        expected: Vec<String>,
    }

    #[test]
    fn shared_tag_hierarchy_vectors_match() {
        let vectors: TagHierarchyVectors =
            serde_json::from_str(include_str!("../../shared-test-vectors/tag-hierarchy.json"))
                .unwrap();

        assert_eq!(
            SEEDED_NAMESPACES,
            vectors.seeded_namespaces.as_slice(),
            "seeded namespaces"
        );

        for case in vectors.parse_cases {
            let parsed = parse_tag(&case.input);
            assert_eq!(parsed.namespace, case.namespace, "{}", case.name);
            assert_eq!(parsed.value, case.value, "{}", case.name);
        }

        for case in vectors.format_cases {
            assert_eq!(
                format_tag(case.namespace.as_deref(), &case.value),
                case.expected,
                "{}",
                case.name
            );
        }

        for case in vectors.normalize_namespace_cases {
            let normalized = case.input.trim().to_lowercase();
            let actual = NAMESPACE_RE.is_match(&normalized).then_some(normalized);
            assert_eq!(actual, case.expected, "{}", case.name);
        }

        for case in vectors.group_cases {
            assert_eq!(group_names(&case.names), case.expected, "{}", case.name);
        }

        for case in vectors.known_namespaces_cases {
            assert_eq!(
                known_namespaces(&case.names),
                case.expected,
                "{}",
                case.name
            );
        }
    }

    fn group_names(names: &[String]) -> Vec<ExpectedGroup> {
        let mut buckets: HashMap<Option<String>, Vec<String>> = HashMap::new();
        for name in names {
            buckets
                .entry(parse_tag(name).namespace)
                .or_default()
                .push(name.clone());
        }

        let mut namespaces: Vec<String> = SEEDED_NAMESPACES
            .iter()
            .filter(|namespace| buckets.contains_key(&Some(namespace.to_string())))
            .map(|namespace| (*namespace).to_string())
            .collect();
        let mut extras: Vec<String> = buckets
            .keys()
            .filter_map(Clone::clone)
            .filter(|namespace| !SEEDED_NAMESPACES.contains(&namespace.as_str()))
            .collect();
        extras.sort();
        namespaces.extend(extras);

        let mut groups: Vec<ExpectedGroup> = namespaces
            .into_iter()
            .map(|namespace| ExpectedGroup {
                names: sorted_by_value(buckets.remove(&Some(namespace.clone())).unwrap()),
                namespace: Some(namespace),
            })
            .collect();
        if let Some(uncategorized) = buckets.remove(&None) {
            groups.push(ExpectedGroup {
                namespace: None,
                names: sorted_by_value(uncategorized),
            });
        }
        groups
    }

    fn sorted_by_value(mut names: Vec<String>) -> Vec<String> {
        names.sort_by_key(|name| parse_tag(name).value.to_lowercase());
        names
    }

    fn known_namespaces(names: &[String]) -> Vec<String> {
        let mut extras: Vec<String> = names
            .iter()
            .filter_map(|name| parse_tag(name).namespace)
            .filter(|namespace| !SEEDED_NAMESPACES.contains(&namespace.as_str()))
            .collect();
        extras.sort();
        extras.dedup();

        SEEDED_NAMESPACES
            .iter()
            .map(|namespace| (*namespace).to_string())
            .chain(extras)
            .collect()
    }

    #[test]
    fn parse_flat_name() {
        let p = parse_tag("dinner");
        assert_eq!(p.namespace, None);
        assert_eq!(p.value, "dinner");
    }

    #[test]
    fn parse_hierarchical() {
        let p = parse_tag("ingredient:chicken");
        assert_eq!(p.namespace, Some("ingredient".to_string()));
        assert_eq!(p.value, "chicken");
    }

    #[test]
    fn parse_multi_colon_stays_flat() {
        let p = parse_tag("a:b:c");
        assert_eq!(p.namespace, None);
        assert_eq!(p.value, "a:b:c");
    }

    #[test]
    fn parse_trims_whitespace() {
        let p = parse_tag("  course : breakfast  ");
        assert_eq!(p.namespace, Some("course".to_string()));
        assert_eq!(p.value, "breakfast");
    }

    #[test]
    fn parse_empty_side_is_flat() {
        assert_eq!(parse_tag(":foo").namespace, None);
        assert_eq!(parse_tag("foo:").namespace, None);
    }

    #[test]
    fn format_without_namespace() {
        assert_eq!(format_tag(None, "dinner"), "dinner");
        assert_eq!(format_tag(Some(""), "dinner"), "dinner");
    }

    #[test]
    fn format_with_namespace() {
        assert_eq!(format_tag(Some("course"), "breakfast"), "course:breakfast");
    }

    #[test]
    fn validate_empty_rejected() {
        assert_eq!(validate_tag_name(""), Err(TagNameError::Empty));
        assert_eq!(validate_tag_name("   "), Err(TagNameError::Empty));
    }

    #[test]
    fn validate_flat_ok() {
        assert!(validate_tag_name("dinner").is_ok());
        assert!(validate_tag_name("Quick Weeknight").is_ok());
    }

    #[test]
    fn validate_hierarchical_ok() {
        assert!(validate_tag_name("ingredient:chicken").is_ok());
        assert!(validate_tag_name("course:breakfast").is_ok());
        assert!(validate_tag_name("diet:gluten-free").is_ok());
    }

    #[test]
    fn validate_multi_colon_rejected() {
        assert_eq!(
            validate_tag_name("a:b:c"),
            Err(TagNameError::MultipleColons)
        );
    }

    #[test]
    fn validate_empty_sides_rejected() {
        assert_eq!(
            validate_tag_name(":chicken"),
            Err(TagNameError::EmptyNamespace)
        );
        assert_eq!(
            validate_tag_name("ingredient:"),
            Err(TagNameError::EmptyValue)
        );
    }

    #[test]
    fn validate_namespace_charset() {
        assert_eq!(
            validate_tag_name("Course:breakfast"),
            Err(TagNameError::InvalidNamespace)
        );
        assert_eq!(
            validate_tag_name("1course:breakfast"),
            Err(TagNameError::InvalidNamespace)
        );
        assert_eq!(
            validate_tag_name("course!:breakfast"),
            Err(TagNameError::InvalidNamespace)
        );
    }

    #[test]
    fn validate_whitespace_around_colon_rejected() {
        assert_eq!(
            validate_tag_name("course : breakfast"),
            Err(TagNameError::WhitespaceAroundColon)
        );
        assert_eq!(
            validate_tag_name("course :breakfast"),
            Err(TagNameError::WhitespaceAroundColon)
        );
        assert_eq!(
            validate_tag_name("course: breakfast"),
            Err(TagNameError::WhitespaceAroundColon)
        );
        assert_eq!(validate_tag_name("  course:breakfast  "), Ok(()));
    }
}
