//! Recover ingredient group headers that structured data drops.
//!
//! JSON-LD `recipeIngredient` is a flat list, so sites that render ingredient
//! groups ("For the sauce", "Dough", …) lose them on import. Rather than
//! re-extracting ingredient text from site-specific markup, this finds the
//! structured lines rendered on the page, in order, and reads the headings that
//! sit between them. The structured lines are kept as-is; only colon-terminated
//! header lines are inserted, which `parse_ingredients` turns into sections.

use super::*;

/// Longest heading text we accept between two ingredients.
const MAX_HEADER_CHARS: usize = 60;

/// How many letters of non-ingredient text we scan past while looking for the
/// next ingredient before giving up on a candidate list. Generous enough for a
/// heading plus some letter-bearing junk, small enough to stop at prose.
const MAX_GAP_LETTERS: usize = 80;

const SKIPPED_ELEMENTS: &[&str] = &["script", "style", "noscript", "template", "head"];

const ROW_ELEMENTS: &[&str] = &["li", "tr", "dd", "p"];

const HEADING_ELEMENTS: &[&str] = &["h1", "h2", "h3", "h4", "h5", "h6", "strong", "b"];

/// One non-whitespace text node of the rendered page.
struct Token<'a> {
    text: &'a str,
    /// Lowercased letters only; see [`match_key`].
    key: String,
    parent: ElementRef<'a>,
}

/// A heading found between ingredients, with the element its text sits in.
struct Heading<'a> {
    name: String,
    element: ElementRef<'a>,
}

/// A structured ingredient line found on the page as tokens `start..end`.
struct MatchedLine {
    start: usize,
    end: usize,
}

/// Whether `ingredients` is a flat list that header recovery could improve.
pub(super) fn needs_ingredient_header_recovery(ingredients: &str) -> bool {
    let mut count = 0;
    for line in ingredients.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if is_header_line(line) {
            return false;
        }
        count += 1;
    }
    count >= 2
}

/// Insert the page's ingredient group headings into a flat ingredient list.
///
/// Returns `None` (leave the list alone) unless every line is found on the page
/// in order, separated only by headings or letterless junk, and at least one
/// heading is found.
pub(super) fn recover_ingredient_headers(ingredients: &str, document: &Html) -> Option<String> {
    if !needs_ingredient_header_recovery(ingredients) {
        return None;
    }
    let lines: Vec<&str> = ingredients
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    let keys: Vec<String> = lines
        .iter()
        .map(|l| match_key(&decode_html_entities(l)))
        .collect();
    if keys.iter().any(String::is_empty) {
        return None;
    }

    let tokens = tokenize(document);
    // A page can render the list more than once (e.g. an ungrouped summary
    // before the grouped card), so keep looking past copies without headings.
    let header_lines = (0..tokens.len())
        .filter_map(|start| match_lines_from(&tokens, &keys, start))
        .find_map(|(matched, headers)| usable_headers(&tokens, &matched, headers))?;

    let mut out = Vec::with_capacity(lines.len() + header_lines.len());
    for (line, header) in lines.iter().zip(header_lines) {
        out.extend(header);
        out.push((*line).to_string());
    }
    Some(out.join("\n"))
}

/// The header line (if any) to insert before each matched line, or `None` if
/// this copy of the list has no headings we can safely use.
fn usable_headers<'a>(
    tokens: &[Token<'a>],
    matched: &[MatchedLine],
    mut headers: Vec<Option<Heading<'a>>>,
) -> Option<Vec<Option<String>>> {
    let items: Vec<ElementRef<'a>> = matched
        .iter()
        .map(|m| {
            tokens[m.start + 1..m.end]
                .iter()
                .fold(tokens[m.start].parent, |acc, t| {
                    common_ancestor(acc, t.parent)
                })
        })
        .collect();
    let container = items[1..]
        .iter()
        .fold(items[0], |acc, &item| common_ancestor(acc, item));

    headers[0] = leading_header(&tokens[..matched[0].start], container);
    if headers.iter().enumerate().any(|(i, h)| {
        h.as_ref()
            .is_some_and(|h| is_inside_ingredient_row(h, &items, i))
    }) {
        return None;
    }
    if has_unheaded_group_after_header(&items, &headers, container) {
        return None;
    }
    let header_lines: Vec<Option<String>> = headers
        .into_iter()
        .map(|h| h.map(|h| format!("{}:", h.name)))
        .collect();
    if header_lines.iter().all(Option::is_none) {
        return None;
    }
    // A header the parser wouldn't recognize (e.g. "Version #1: Garden
    // Vegetable:") would become a bogus ingredient, and dropping just that one
    // would file its group under the previous heading.
    if header_lines
        .iter()
        .flatten()
        .any(|h| detect_section_header(h).is_none())
    {
        return None;
    }
    Some(header_lines)
}

fn is_header_line(line: &str) -> bool {
    line.ends_with(':') || detect_section_header(line).is_some()
}

/// Lowercased letters only. Spacing, punctuation, digits, and fraction style
/// all vary between structured data and the rendered page ("1/2 cup onion ,
/// sliced" vs "½ cup onion, sliced"), so they're ignored when matching.
fn match_key(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphabetic())
        .flat_map(char::to_lowercase)
        .collect()
}

fn tokenize(document: &Html) -> Vec<Token<'_>> {
    document
        .root_element()
        .descendants()
        .filter_map(|node| {
            let text: &str = node.value().as_text()?;
            if text.trim().is_empty() {
                return None;
            }
            let parent = node.parent().and_then(ElementRef::wrap)?;
            let skipped = std::iter::once(parent)
                .chain(parent.ancestors().filter_map(ElementRef::wrap))
                .any(|el| SKIPPED_ELEMENTS.contains(&el.value().name()));
            if skipped {
                return None;
            }
            Some(Token {
                text,
                key: match_key(text),
                parent,
            })
        })
        .collect()
}

/// Find every line's tokens, in order, with the first line starting at token
/// `start`. Returns the matches plus the heading (if any) in the gap before
/// each line; the slot for the first line is left `None` for the caller.
fn match_lines_from<'a>(
    tokens: &[Token<'a>],
    keys: &[String],
    start: usize,
) -> Option<(Vec<MatchedLine>, Vec<Option<Heading<'a>>>)> {
    let end = match_run(tokens, start, &keys[0])?;
    let mut matched = vec![MatchedLine { start, end }];
    let mut headers = vec![None];
    for key in &keys[1..] {
        let gap_start = matched.last().map_or(0, |m| m.end);
        let next = find_next_line(tokens, gap_start, key)?;
        headers.push(gap_header(&tokens[gap_start..next.start]).ok()?);
        matched.push(next);
    }
    Some((matched, headers))
}

/// Match `key` against consecutive tokens starting at `start`.
fn match_run(tokens: &[Token<'_>], start: usize, key: &str) -> Option<usize> {
    if tokens.get(start)?.key.is_empty() {
        return None;
    }
    let mut matched = 0;
    for (i, token) in tokens.iter().enumerate().skip(start) {
        let rest = key.get(matched..)?;
        if !rest.starts_with(token.key.as_str()) {
            return None;
        }
        matched += token.key.len();
        if matched == key.len() {
            return Some(i + 1);
        }
    }
    None
}

fn find_next_line(tokens: &[Token<'_>], from: usize, key: &str) -> Option<MatchedLine> {
    let mut gap_letters = 0;
    for start in from..tokens.len() {
        if let Some(end) = match_run(tokens, start, key) {
            return Some(MatchedLine { start, end });
        }
        gap_letters += tokens[start].key.len();
        if gap_letters > MAX_GAP_LETTERS {
            return None;
        }
    }
    None
}

/// Classify the tokens between two ingredients: `Ok(None)` for letterless
/// junk (checkboxes, bullets, amounts), `Ok(Some(_))` for a single heading,
/// `Err` for anything else.
fn gap_header<'a>(gap: &[Token<'a>]) -> Result<Option<Heading<'a>>, ()> {
    let worded: Vec<&Token<'_>> = gap.iter().filter(|t| !t.key.is_empty()).collect();
    let Some(first) = worded.first() else {
        return Ok(None);
    };
    let text = normalize_header_text(worded.iter().map(|t| t.text));
    let same_heading = heading_element(first.parent).is_some_and(|heading| {
        worded
            .iter()
            .all(|t| heading_element(t.parent) == Some(heading))
    });
    let colon_label = text.ends_with(':') && worded.iter().all(|t| t.parent == first.parent);
    if !(same_heading || colon_label) {
        return Err(());
    }
    let name = text.trim_end_matches(':').trim();
    let key = match_key(name);
    if key.is_empty() || key == "ingredients" || name.chars().count() > MAX_HEADER_CHARS {
        return Err(());
    }
    Ok(Some(Heading {
        name: name.to_string(),
        element: first.parent,
    }))
}

/// The heading right before the first ingredient, if it belongs to the list.
/// Only the closest heading or colon label counts, and it must sit inside the
/// element that holds every ingredient, so list titles ("Ingredients") and
/// controls above the list (unit toggles, scaling buttons) are skipped.
fn leading_header<'a>(before: &[Token<'a>], container: ElementRef<'a>) -> Option<Heading<'a>> {
    // A heading's text can span several nodes (`<h3>Sauce <em>B</em></h3>`);
    // a plain colon label is one element's text.
    let label_element = |t: &Token<'a>| heading_element(t.parent).unwrap_or(t.parent);
    let last_worded = before.iter().rposition(|t| !t.key.is_empty())?;
    let label = label_element(&before[last_worded]);
    if !contains(container, label) || label == container {
        return None;
    }
    let first = before[..=last_worded]
        .iter()
        .rposition(|t| !t.key.is_empty() && label_element(t) != label)
        .map_or(0, |i| i + 1);
    gap_header(&before[first..=last_worded]).ok().flatten()
}

/// The heading-like element (the text's parent or grandparent) a text node
/// belongs to, e.g. `<h3>` or `<p><strong>`.
fn heading_element(parent: ElementRef<'_>) -> Option<ElementRef<'_>> {
    std::iter::once(parent)
        .chain(parent.parent().and_then(ElementRef::wrap))
        .find(|el| HEADING_ELEMENTS.contains(&el.value().name()))
}

/// Bold text inside an ingredient's own row is an annotation the structured
/// line omitted, not a heading: `<li>1 cup flour <strong>King Arthur
/// preferred</strong></li>` or `<li><strong>Optional:</strong> 1 cup
/// nuts</li>`. Text inside an element that holds both neighbors
/// (`<p>flour<br><b>Glaze</b><br>sugar</p>`) can still be one.
fn is_inside_ingredient_row(
    heading: &Heading<'_>,
    items: &[ElementRef<'_>],
    before: usize,
) -> bool {
    // Nothing that follows an ingredient inside a wrapper of its own can
    // introduce the next group, whatever the wrapper is.
    let after_prev = before.checked_sub(1).is_some_and(|prev| {
        own_wrappers(prev, items)
            .last()
            .is_some_and(|&wrapper| contains(wrapper, heading.element))
    });
    // Before an ingredient it's ambiguous: a `<div>` holding a heading and one
    // ingredient is a group (ATK), so only list-like rows count there.
    let before_next = before < items.len()
        && own_wrappers(before, items)
            .into_iter()
            .find(|el| ROW_ELEMENTS.contains(&el.value().name()))
            .is_some_and(|row| contains(row, heading.element));
    after_prev || before_next
}

/// `item` and its ancestors that hold no other matched ingredient, innermost
/// first.
fn own_wrappers<'a>(index: usize, items: &[ElementRef<'a>]) -> Vec<ElementRef<'a>> {
    let item = items[index];
    std::iter::once(item)
        .chain(item.ancestors().filter_map(ElementRef::wrap))
        .take_while(|el| {
            items
                .iter()
                .enumerate()
                .all(|(i, &other)| i == index || !contains(*el, other))
        })
        .collect()
}

fn contains(outer: ElementRef<'_>, inner: ElementRef<'_>) -> bool {
    outer == inner || inner.ancestors().any(|a| a.id() == outer.id())
}

/// The flat ingredient text can't close a section, so an unheaded group that
/// follows a headed one would be filed under the earlier heading. When the page
/// really groups its ingredients (some group holds several, or a heading sits
/// inside the same wrapper as the ingredient it introduces), require a heading
/// at every group change once headings have started.
fn has_unheaded_group_after_header(
    items: &[ElementRef<'_>],
    headers: &[Option<Heading<'_>>],
    container: ElementRef<'_>,
) -> bool {
    let groups: Vec<_> = items
        .iter()
        .map(|&item| group_of(item, container))
        .collect();
    let shared_group = groups.windows(2).any(|w| w[0].is_some() && w[0] == w[1]);
    let wrapped_heading = headers.iter().zip(&groups).any(|(header, group)| {
        header
            .as_ref()
            .is_some_and(|h| group.is_some() && group_of(h.element, container) == *group)
    });
    if !(shared_group || wrapped_heading) {
        return false;
    }
    let mut seen_header = false;
    for (i, header) in headers.iter().enumerate() {
        if header.is_some() {
            seen_header = true;
        } else if seen_header && groups[i] != groups[i - 1] {
            return true;
        }
    }
    false
}

/// The child of `container` that holds `item`.
fn group_of<'a>(item: ElementRef<'a>, container: ElementRef<'a>) -> Option<ElementRef<'a>> {
    std::iter::once(item)
        .chain(item.ancestors().filter_map(ElementRef::wrap))
        .find(|el| el.parent().is_some_and(|p| p.id() == container.id()))
}

fn common_ancestor<'a>(a: ElementRef<'a>, b: ElementRef<'a>) -> ElementRef<'a> {
    let a_chain: Vec<_> = std::iter::once(*a)
        .chain(a.ancestors())
        .map(|n| n.id())
        .collect();
    std::iter::once(b)
        .chain(b.ancestors().filter_map(ElementRef::wrap))
        .find(|el| a_chain.contains(&el.id()))
        .unwrap_or(b)
}

fn normalize_header_text<'a>(parts: impl Iterator<Item = &'a str>) -> String {
    parts
        .flat_map(str::split_whitespace)
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recover(ingredients: &[&str], body: &str) -> Option<Vec<String>> {
        let document = Html::parse_document(&format!("<html><body>{body}</body></html>"));
        recover_ingredient_headers(&ingredients.join("\n"), &document)
            .map(|s| s.lines().map(String::from).collect())
    }

    const ATK_INGREDIENTS: &[&str] = &[
        "1 red onion , halved and sliced thin",
        "1/3 cup sugar ",
        "2/3 cup mayonnaise ",
        "1 tablespoon Tapatío hot sauce ",
    ];

    fn atk_group(name: Option<&str>, items: &[&str]) -> String {
        let heading = name.map_or(String::new(), |n| {
            format!(r#"<h3 class="mise-text">{n}</h3>"#)
        });
        let items: String = items
            .iter()
            .map(|i| format!(r#"<span class="ingredient"><span>{i}</span></span>"#))
            .collect();
        format!("<div>{heading}{items}</div>")
    }

    #[test]
    fn atk_groups_become_headers_on_structured_lines() {
        let body = format!(
            r#"<h4>Ingredients</h4><div class="list">{}{}</div>"#,
            atk_group(
                Some("Pickled Red Onion"),
                &["1 red onion, halved and sliced thin", "⅓ cup sugar"]
            ),
            atk_group(
                Some("Zarandeado Sauce"),
                &[
                    "⅔ cup </span><span>mayonnaise</span><span> ",
                    "1 tablespoon Tapatío hot sauce"
                ]
            ),
        );
        assert_eq!(
            recover(ATK_INGREDIENTS, &body).unwrap(),
            vec![
                "Pickled Red Onion:",
                "1 red onion , halved and sliced thin",
                "1/3 cup sugar",
                "Zarandeado Sauce:",
                "2/3 cup mayonnaise",
                "1 tablespoon Tapatío hot sauce",
            ]
        );
    }

    #[test]
    fn trailing_unheaded_group_bails() {
        let body = format!(
            r#"<div class="list">{}{}</div>"#,
            atk_group(
                Some("Onion"),
                &["1 red onion, halved and sliced thin", "⅓ cup sugar"]
            ),
            atk_group(
                None,
                &["⅔ cup mayonnaise", "1 tablespoon Tapatío hot sauce"]
            ),
        );
        assert_eq!(recover(ATK_INGREDIENTS, &body), None);
    }

    #[test]
    fn unheaded_singleton_group_after_headed_one_bails() {
        let body = format!(
            r#"<div class="list">{}{}</div>"#,
            atk_group(Some("Onion"), &["1 red onion, halved and sliced thin"]),
            atk_group(None, &["⅓ cup sugar"]),
        );
        assert_eq!(recover(&ATK_INGREDIENTS[..2], &body), None);
    }

    #[test]
    fn leading_colon_label_is_a_header() {
        let body = r#"
            <ul>
              <li>Dough:</li>
              <li>2 cups flour</li>
              <li>Glaze:</li>
              <li>1 cup powdered sugar</li>
            </ul>"#;
        assert_eq!(
            recover(&["2 cups flour", "1 cup powdered sugar"], body).unwrap(),
            vec!["Dough:", "2 cups flour", "Glaze:", "1 cup powdered sugar"]
        );
    }

    #[test]
    fn bold_annotation_inside_an_ingredient_row_bails() {
        let body = r#"
            <ul>
              <li>2 cups flour <strong>King Arthur preferred</strong></li>
              <li>1 cup butter</li>
            </ul>"#;
        assert_eq!(recover(&["2 cups flour", "1 cup butter"], body), None);
    }

    #[test]
    fn bold_label_leading_an_ingredient_row_bails() {
        let body = r#"
            <ul>
              <li>2 cups flour</li>
              <li><strong>Optional:</strong> 1 cup nuts</li>
            </ul>"#;
        assert_eq!(recover(&["2 cups flour", "1 cup nuts"], body), None);
    }

    #[test]
    fn bold_label_beside_a_wrapped_ingredient_bails() {
        let body = r#"
            <ul>
              <li><span>2 cups flour</span></li>
              <li><strong>Optional:</strong><span>1 cup nuts</span></li>
            </ul>"#;
        assert_eq!(recover(&["2 cups flour", "1 cup nuts"], body), None);
    }

    #[test]
    fn bold_annotation_after_a_wrapped_ingredient_bails() {
        let body = r#"
            <p><span>2 cups flour</span><strong>King Arthur preferred</strong></p>
            <div><span>1 cup butter</span></div>"#;
        assert_eq!(recover(&["2 cups flour", "1 cup butter"], body), None);
    }

    #[test]
    fn bold_heading_inside_a_shared_paragraph_is_kept() {
        let body = r#"<p>2 cups flour<br><b>Glaze</b><br>1 cup powdered sugar</p>"#;
        assert_eq!(
            recover(&["2 cups flour", "1 cup powdered sugar"], body).unwrap(),
            vec!["2 cups flour", "Glaze:", "1 cup powdered sugar"]
        );
    }

    #[test]
    fn leading_unheaded_group_is_fine() {
        let body = format!(
            r#"<div class="list">{}{}</div>"#,
            atk_group(
                None,
                &["1 red onion, halved and sliced thin", "⅓ cup sugar"]
            ),
            atk_group(
                Some("Sauce"),
                &["⅔ cup mayonnaise", "1 tablespoon Tapatío hot sauce"]
            ),
        );
        assert_eq!(
            recover(ATK_INGREDIENTS, &body).unwrap(),
            vec![
                "1 red onion , halved and sliced thin",
                "1/3 cup sugar",
                "Sauce:",
                "2/3 cup mayonnaise",
                "1 tablespoon Tapatío hot sauce",
            ]
        );
    }

    #[test]
    fn wprm_shaped_list_skips_title_and_unit_toggle() {
        let body = r#"
            <div class="wprm-recipe-ingredients-container">
              <h3>Ingredients</h3>
              <div class="toggle"><a>US Customary</a> - <a>Metric</a></div>
              <div class="wprm-recipe-ingredient-group">
                <h4 class="wprm-recipe-group-name">Meatballs</h4>
                <ul>
                  <li><span>▢</span> <span>1</span> <span>lb</span> <span>ground beef</span></li>
                  <li><span>▢</span> <span>1</span> <span>egg</span></li>
                </ul>
              </div>
              <div class="wprm-recipe-ingredient-group">
                <h4 class="wprm-recipe-group-name">Broth:</h4>
                <ul>
                  <li><span>▢</span> <span>4</span> <span>cups</span> <span>stock</span></li>
                </ul>
              </div>
            </div>"#;
        assert_eq!(
            recover(&["1 lb ground beef", "1 egg", "4 cups stock"], body).unwrap(),
            vec![
                "Meatballs:",
                "1 lb ground beef",
                "1 egg",
                "Broth:",
                "4 cups stock"
            ]
        );
    }

    #[test]
    fn flat_list_with_bold_header_items() {
        let body = r#"
            <ul>
              <li>2 cups flour</li>
              <li>1 tsp salt</li>
              <li><strong>Glaze</strong></li>
              <li>1 cup powdered sugar</li>
            </ul>"#;
        assert_eq!(
            recover(
                &["2 cups flour", "1 tsp salt", "1 cup powdered sugar"],
                body
            )
            .unwrap(),
            vec![
                "2 cups flour",
                "1 tsp salt",
                "Glaze:",
                "1 cup powdered sugar"
            ]
        );
    }

    #[test]
    fn prose_mention_before_the_list_is_skipped() {
        let body = r#"
            <p>2 cups flour</p>
            <p>is the base of this cake, and you can swap in whole wheat flour if you
               like a heartier crumb with more texture.</p>
            <ul>
              <li>2 cups flour</li>
              <li><b>Frosting</b></li>
              <li>1 cup butter</li>
            </ul>"#;
        assert_eq!(
            recover(&["2 cups flour", "1 cup butter"], body).unwrap(),
            vec!["2 cups flour", "Frosting:", "1 cup butter"]
        );
    }

    #[test]
    fn non_heading_text_between_lines_bails() {
        let body = r#"
            <ul>
              <li>2 cups flour</li>
              <li><a href="/shop">Buy flour</a></li>
              <li><strong>Frosting</strong></li>
              <li>1 cup butter</li>
            </ul>"#;
        assert_eq!(recover(&["2 cups flour", "1 cup butter"], body), None);
    }

    #[test]
    fn structured_headers_are_left_alone() {
        let body = r#"<ul><li><b>Cake</b></li><li>2 cups flour</li><li><b>Icing</b></li><li>1 cup butter</li></ul>"#;
        assert_eq!(
            recover(&["For the cake:", "2 cups flour", "1 cup butter"], body),
            None
        );
    }

    #[test]
    fn extract_recipe_recovers_headers_on_the_jsonld_fast_path() {
        let html = r#"<html><head><script type="application/ld+json">
            {"@type": "Recipe", "name": "Tacos",
             "recipeIngredient": ["1 red onion , sliced", "1/2 cup mayonnaise"],
             "recipeInstructions": "Make tacos."}
            </script></head><body>
            <div><div><h3>Pickles</h3><span>1 red onion, sliced</span></div>
            <div><h3>Sauce</h3><span>½ cup mayonnaise</span></div></div>
            </body></html>"#;
        let recipe = extract_recipe(html, "https://example.com/tacos").unwrap();
        assert_eq!(
            recipe.ingredients,
            "Pickles:\n1 red onion , sliced\nSauce:\n1/2 cup mayonnaise"
        );
    }

    #[test]
    fn ungrouped_copy_before_grouped_list_is_skipped() {
        let body = r#"
            <ul class="summary"><li>2 cups flour</li><li>1 cup butter</li></ul>
            <ul class="card">
              <li>2 cups flour</li>
              <li><strong>Frosting</strong></li>
              <li>1 cup butter</li>
            </ul>"#;
        assert_eq!(
            recover(&["2 cups flour", "1 cup butter"], body).unwrap(),
            vec!["2 cups flour", "Frosting:", "1 cup butter"]
        );
    }

    #[test]
    fn header_the_parser_rejects_bails() {
        let body = r#"
            <ul>
              <li><strong>Base Frittata Recipe</strong></li>
              <li>8 large eggs</li>
              <li><strong>Version #1: Garden Vegetable</strong></li>
              <li>1 cup spinach</li>
            </ul>"#;
        assert_eq!(recover(&["8 large eggs", "1 cup spinach"], body), None);
    }

    #[test]
    fn extract_recipe_recovers_headers_on_the_html_fallback_path() {
        // No recipeInstructions, so the full JSON-LD extractors fail and the
        // HTML fallback merges JSON-LD ingredients with page instructions.
        let html = r#"<html><head><script type="application/ld+json">
            {"@type": "Recipe", "name": "Tacos",
             "recipeIngredient": ["1 red onion , sliced", "1/2 cup mayonnaise"]}
            </script></head><body>
            <div><div><h3>Pickles</h3><span>1 red onion, sliced</span></div>
            <div><h3>Sauce</h3><span>½ cup mayonnaise</span></div></div>
            <div class="recipe-instructions"><p>Make tacos.</p></div>
            </body></html>"#;
        let recipe = extract_recipe(html, "https://example.com/tacos").unwrap();
        assert_eq!(
            recipe.ingredients,
            "Pickles:\n1 red onion , sliced\nSauce:\n1/2 cup mayonnaise"
        );
    }

    #[test]
    fn no_headings_returns_none() {
        let body = r#"<ul><li>2 cups flour</li><li>1 cup butter</li></ul>"#;
        assert_eq!(recover(&["2 cups flour", "1 cup butter"], body), None);
    }
}
