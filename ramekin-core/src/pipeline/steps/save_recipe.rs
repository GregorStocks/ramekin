//! SaveRecipe step metadata - abstract in core, implemented in CLI/server.

use crate::pipeline::StepMetadata;

/// Metadata for the SaveRecipe step.
///
/// This step is abstract in core (no execute implementation) because it needs
/// DB access on server (to create Recipe records) and different behavior on CLI
/// (just saves to file). CLI and server must provide their own implementations.
pub struct SaveRecipeStepMeta;

impl SaveRecipeStepMeta {
    /// Step name constant.
    pub const NAME: &'static str = "save_recipe";

    /// Get metadata for this step.
    pub fn metadata() -> StepMetadata {
        StepMetadata {
            name: Self::NAME,
            description: "Save recipe to storage",
            continues_on_failure: false,
        }
    }
}
