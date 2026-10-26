//! Single source of truth for the operations that derive a recipe version.
//!
//! A version comes from a source plus operations applied to it. Source ops
//! replace the base content and so come first; normalize ops transform
//! whatever content the source ops (or the current version) produced. Any
//! subset can be re-run on a recipe at any time, and an import runs the
//! source plus [`default_import_ops`].

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::ai::{AiClient, AiConfig, CachingAiClient, ConfigError, UnconfiguredAiClient};
use crate::pipeline::steps::{
    AutoTagStep, GenerateDescriptionStep, HouseStyleStep, NormalizeTitleStep,
};
use crate::pipeline::PipelineStep;

/// Step that writes the recipe's current version as a `raw_recipe` output,
/// for jobs that normalize without re-reading the source. Normalize steps
/// read it in place of `extract_recipe` (see `steps::base_raw_recipe`).
pub const LOAD_RECIPE_STEP: &str = "load_recipe";

/// Resolves the saved recipe's ingredient names the catalog doesn't know.
pub const RESOLVE_INGREDIENT_NAMES_STEP: &str = "resolve_ingredient_names";

/// Writes one new version with every normalize op's output on top of a
/// version the job just saved (imports, scrapes).
pub const APPLY_NORMALIZE_STEP: &str = "apply_normalize";

/// Builds the content a manual normalize run proposes, without saving it.
pub const PROPOSE_NORMALIZE_STEP: &str = "propose_normalize";

/// Edits the loaded recipe the way an Edit with AI job's request says, for
/// `propose_normalize` to propose.
pub const EDIT_WITH_AI_STEP: &str = "edit_with_ai";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NormalizeOp {
    /// Download the source URL again and rebuild the recipe from it.
    Refetch,
    /// Rebuild the recipe from its stored source with the current extractor
    /// and ingredient parser.
    Reextract,
    NormalizeTitle,
    GenerateDescription,
    AutoTag,
    /// Rewrite ingredients and steps to `doc/house-style.md`.
    HouseStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NormalizeOpGroup {
    Source,
    Normalize,
}

/// Every content field a source op rebuilds.
const SOURCE_FIELDS: &[&str] = &[
    "title",
    "description",
    "ingredients",
    "instructions",
    "source_name",
    "servings",
    "prep_time",
    "cook_time",
    "total_time",
    "rating",
    "difficulty",
    "nutritional_info",
    "notes",
];

const REFETCH_FIELDS: &[&str] = &[
    "title",
    "description",
    "ingredients",
    "instructions",
    "source_name",
    "servings",
    "prep_time",
    "cook_time",
    "total_time",
    "rating",
    "difficulty",
    "nutritional_info",
    "notes",
    "photo_ids",
];

impl NormalizeOp {
    /// Every op in canonical run order. Add new ops here.
    pub const ALL: &'static [NormalizeOp] = &[
        NormalizeOp::Refetch,
        NormalizeOp::Reextract,
        NormalizeOp::NormalizeTitle,
        NormalizeOp::GenerateDescription,
        NormalizeOp::AutoTag,
        NormalizeOp::HouseStyle,
    ];

    pub fn id(self) -> &'static str {
        match self {
            NormalizeOp::Refetch => "refetch",
            NormalizeOp::Reextract => "reextract",
            NormalizeOp::NormalizeTitle => "normalize_title",
            NormalizeOp::GenerateDescription => "generate_description",
            NormalizeOp::AutoTag => "auto_tag",
            NormalizeOp::HouseStyle => "house_style",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|op| op.id() == id)
    }

    pub fn label(self) -> &'static str {
        match self {
            NormalizeOp::Refetch => "Re-fetch from source",
            NormalizeOp::Reextract => "Re-extract from saved source",
            NormalizeOp::NormalizeTitle => "Normalize title",
            NormalizeOp::GenerateDescription => "Generate description",
            NormalizeOp::AutoTag => "Auto-tag",
            NormalizeOp::HouseStyle => "Apply house style",
        }
    }

    pub fn group(self) -> NormalizeOpGroup {
        match self {
            NormalizeOp::Refetch | NormalizeOp::Reextract => NormalizeOpGroup::Source,
            NormalizeOp::NormalizeTitle
            | NormalizeOp::GenerateDescription
            | NormalizeOp::AutoTag
            | NormalizeOp::HouseStyle => NormalizeOpGroup::Normalize,
        }
    }

    pub fn uses_ai(self) -> bool {
        self.group() == NormalizeOpGroup::Normalize
    }

    /// Recipe fields the op can change.
    pub fn writes(self) -> &'static [&'static str] {
        match self {
            NormalizeOp::Refetch => REFETCH_FIELDS,
            NormalizeOp::Reextract => SOURCE_FIELDS,
            NormalizeOp::NormalizeTitle => &["title"],
            NormalizeOp::GenerateDescription => &["description"],
            NormalizeOp::AutoTag => &["tags"],
            NormalizeOp::HouseStyle => &["ingredients", "instructions"],
        }
    }

    /// Ops whose output this one reads, so it must run after them when both
    /// run.
    pub fn runs_after(self) -> &'static [NormalizeOp] {
        match self {
            // The description prompt uses the normalized title.
            NormalizeOp::GenerateDescription => &[NormalizeOp::NormalizeTitle],
            NormalizeOp::Refetch
            | NormalizeOp::Reextract
            | NormalizeOp::NormalizeTitle
            | NormalizeOp::AutoTag
            | NormalizeOp::HouseStyle => &[],
        }
    }

    /// Ops that always run with this one.
    pub fn implies(self) -> &'static [NormalizeOp] {
        match self {
            NormalizeOp::Refetch => &[NormalizeOp::Reextract],
            NormalizeOp::Reextract
            | NormalizeOp::NormalizeTitle
            | NormalizeOp::GenerateDescription
            | NormalizeOp::AutoTag
            | NormalizeOp::HouseStyle => &[],
        }
    }

    /// Whether imports and scrapes run the op automatically.
    pub fn default_on_import(self) -> bool {
        self.group() == NormalizeOpGroup::Normalize
    }

    /// The pipeline step a normalize op runs as. Source ops instead decide
    /// where the job's linear steps start.
    pub fn step_name(self) -> Option<&'static str> {
        match self {
            NormalizeOp::Refetch | NormalizeOp::Reextract => None,
            NormalizeOp::NormalizeTitle => Some(NormalizeTitleStep::NAME),
            NormalizeOp::GenerateDescription => Some(GenerateDescriptionStep::NAME),
            NormalizeOp::AutoTag => Some(AutoTagStep::NAME),
            NormalizeOp::HouseStyle => Some(HouseStyleStep::NAME),
        }
    }

    /// Whether the op is a long tool loop rather than one model call: too
    /// slow to run inside a request.
    pub fn long_running(self) -> bool {
        matches!(self, NormalizeOp::HouseStyle)
    }

    /// The step implementation for a normalize op, calling the model the op
    /// uses. A missing AI configuration still gives a step, which fails on
    /// its first call, so it fails like any other AI error.
    pub fn step(
        self,
        ai_config: &Result<AiConfig, ConfigError>,
        user_tags: &[String],
    ) -> Option<Box<dyn PipelineStep>> {
        let client = |configure: fn(&AiConfig) -> AiConfig| -> Arc<dyn AiClient> {
            match ai_config {
                Ok(config) => Arc::new(CachingAiClient::new(configure(config))),
                Err(e) => Arc::new(UnconfiguredAiClient::new(e.clone())),
            }
        };
        match self {
            NormalizeOp::Refetch | NormalizeOp::Reextract => None,
            NormalizeOp::NormalizeTitle => {
                Some(Box::new(NormalizeTitleStep::new(client(AiConfig::clone))))
            }
            NormalizeOp::GenerateDescription => Some(Box::new(GenerateDescriptionStep::new(
                client(AiConfig::clone),
            ))),
            NormalizeOp::AutoTag => Some(Box::new(AutoTagStep::new(
                client(AiConfig::clone),
                user_tags.to_vec(),
            ))),
            // A tool loop, on the Edit with AI model.
            NormalizeOp::HouseStyle => Some(Box::new(HouseStyleStep::new(client(
                AiConfig::for_recipe_edit,
            )))),
        }
    }

    fn canonical_index(self) -> usize {
        Self::ALL
            .iter()
            .position(|op| *op == self)
            .expect("every op is in NormalizeOp::ALL")
    }
}

/// `ops` plus the ops they imply, without duplicates, in canonical order.
pub fn order_ops(ops: &[NormalizeOp]) -> Vec<NormalizeOp> {
    let mut ordered: Vec<NormalizeOp> = Vec::new();
    let mut pending: Vec<NormalizeOp> = ops.to_vec();
    while let Some(op) = pending.pop() {
        if !ordered.contains(&op) {
            ordered.push(op);
            pending.extend_from_slice(op.implies());
        }
    }
    ordered.sort_by_key(|op| op.canonical_index());
    ordered
}

/// The normalize ops imports and scrapes apply automatically.
pub fn default_import_ops() -> Vec<NormalizeOp> {
    NormalizeOp::ALL
        .iter()
        .copied()
        .filter(|op| op.default_on_import())
        .collect()
}

/// The concurrent phase of a job, in canonical order, each step with the
/// steps it must wait for: every normalize op in `ops` waits for the ops it
/// runs after, then `final_step` waits for all of them. Source ops in `ops`
/// are ignored; they run before this phase.
pub fn normalize_phase_steps(
    ops: &[NormalizeOp],
    final_step: &'static str,
) -> Vec<(&'static str, Vec<&'static str>)> {
    let ops = order_ops(ops);
    let mut steps: Vec<(&'static str, Vec<&'static str>)> = ops
        .iter()
        .filter_map(|op| {
            let name = op.step_name()?;
            let deps = op
                .runs_after()
                .iter()
                .filter(|after| ops.contains(after))
                .filter_map(|after| after.step_name())
                .collect();
            Some((name, deps))
        })
        .collect();
    let all: Vec<&'static str> = steps.iter().map(|(name, _)| *name).collect();
    steps.push((final_step, all));
    steps
}

/// The normalize op that runs as `step_name`, if any.
pub fn op_for_step(step_name: &str) -> Option<NormalizeOp> {
    NormalizeOp::ALL
        .iter()
        .copied()
        .find(|op| op.step_name() == Some(step_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip_and_match_serde() {
        for op in NormalizeOp::ALL {
            assert_eq!(NormalizeOp::from_id(op.id()), Some(*op));
            assert_eq!(
                serde_json::to_value(op).unwrap(),
                serde_json::Value::String(op.id().to_string())
            );
        }
        assert_eq!(NormalizeOp::from_id("enrich"), None);
    }

    #[test]
    fn order_ops_adds_implied_ops_and_sorts() {
        assert_eq!(
            order_ops(&[
                NormalizeOp::AutoTag,
                NormalizeOp::Refetch,
                NormalizeOp::AutoTag
            ]),
            vec![
                NormalizeOp::Refetch,
                NormalizeOp::Reextract,
                NormalizeOp::AutoTag
            ]
        );
    }

    #[test]
    fn runs_after_points_backwards() {
        // Canonical order must be a valid run order.
        for op in NormalizeOp::ALL {
            for before in op.runs_after() {
                assert!(
                    before.canonical_index() < op.canonical_index(),
                    "{op:?} runs after {before:?}, which does not come before it"
                );
            }
        }
    }

    #[test]
    fn source_ops_come_before_normalize_ops() {
        let groups: Vec<_> = NormalizeOp::ALL.iter().map(|op| op.group()).collect();
        let first_normalize = groups
            .iter()
            .position(|g| *g == NormalizeOpGroup::Normalize)
            .unwrap();
        assert!(groups[first_normalize..]
            .iter()
            .all(|g| *g == NormalizeOpGroup::Normalize));
    }

    #[test]
    fn default_import_phase() {
        assert_eq!(
            normalize_phase_steps(&default_import_ops(), APPLY_NORMALIZE_STEP),
            vec![
                ("normalize_title", vec![]),
                ("generate_description", vec!["normalize_title"]),
                ("auto_tag", vec![]),
                ("house_style", vec![]),
                (
                    "apply_normalize",
                    vec![
                        "normalize_title",
                        "generate_description",
                        "auto_tag",
                        "house_style"
                    ]
                ),
            ]
        );
    }

    #[test]
    fn description_without_title_has_no_dependency() {
        assert_eq!(
            normalize_phase_steps(
                &[NormalizeOp::Reextract, NormalizeOp::GenerateDescription],
                PROPOSE_NORMALIZE_STEP
            ),
            vec![
                ("generate_description", vec![]),
                ("propose_normalize", vec!["generate_description"]),
            ]
        );
    }
}
