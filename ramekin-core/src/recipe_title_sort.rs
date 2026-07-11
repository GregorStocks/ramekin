use std::cmp::Ordering;
use uuid::Uuid;

/// Compares recipe titles without relying on a platform or database locale.
///
/// Each Unicode scalar is lowercased independently, then the resulting scalar
/// sequences are compared lexicographically. UUIDs always break equal folded
/// titles in ascending order, including for a descending title sort.
pub fn compare_recipe_titles(
    lhs_title: &str,
    lhs_id: &Uuid,
    rhs_title: &str,
    rhs_id: &Uuid,
    descending: bool,
) -> Ordering {
    let title_order = if descending {
        compare_folded_titles(rhs_title, lhs_title)
    } else {
        compare_folded_titles(lhs_title, rhs_title)
    };

    title_order.then_with(|| lhs_id.cmp(rhs_id))
}

fn compare_folded_titles(lhs: &str, rhs: &str) -> Ordering {
    lhs.chars()
        .flat_map(char::to_lowercase)
        .cmp(rhs.chars().flat_map(char::to_lowercase))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Vectors {
        recipes: Vec<Recipe>,
        ascending: Vec<Uuid>,
        descending: Vec<Uuid>,
    }

    #[derive(Deserialize)]
    struct Recipe {
        id: Uuid,
        title: String,
    }

    #[test]
    fn matches_shared_vectors() {
        let vectors: Vectors = serde_json::from_str(include_str!(
            "../../shared-test-vectors/recipe-title-sort.json"
        ))
        .expect("recipe title sort vectors should be valid");

        assert_eq!(sorted_ids(&vectors.recipes, false), vectors.ascending);
        assert_eq!(sorted_ids(&vectors.recipes, true), vectors.descending);
    }

    fn sorted_ids(recipes: &[Recipe], descending: bool) -> Vec<Uuid> {
        let mut recipes: Vec<&Recipe> = recipes.iter().collect();
        recipes.sort_by(|lhs, rhs| {
            compare_recipe_titles(&lhs.title, &lhs.id, &rhs.title, &rhs.id, descending)
        });
        recipes.into_iter().map(|recipe| recipe.id).collect()
    }
}
