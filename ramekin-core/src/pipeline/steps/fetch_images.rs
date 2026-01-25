//! FetchImages step metadata - abstract in core, implemented in CLI/server.

use crate::pipeline::StepMetadata;

/// Metadata for the FetchImages step.
///
/// This step is abstract in core (no execute implementation) because it needs
/// DB access to create Photo records. CLI and server must provide their own
/// implementations.
pub struct FetchImagesStepMeta;

impl FetchImagesStepMeta {
    /// Step name constant.
    pub const NAME: &'static str = "fetch_images";

    /// Get metadata for this step.
    pub fn metadata() -> StepMetadata {
        StepMetadata {
            name: Self::NAME,
            description: "Fetch and store recipe images",
            continues_on_failure: false,
        }
    }
}
