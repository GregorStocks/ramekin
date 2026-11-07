//! How long each step may run. A step still running at its deadline is
//! stopped and fails the job at that step with a timeout error, which Retry
//! resumes like any other failure; without this a hung step would leave its
//! job running until the server restarts.

use std::future::Future;
use std::time::Duration;

use ramekin_core::pipeline::steps::{
    FetchImagesStepMeta, ParseIngredientsStep, SaveRecipeStepMeta,
};
use ramekin_core::pipeline::{
    StepResult, APPLY_NORMALIZE_STEP, LOAD_RECIPE_STEP, PROPOSE_NORMALIZE_STEP,
};

use super::steps::FetchHtmlStep;

/// Local work and database writes.
const LOCAL_STEP_DEADLINE: Duration = Duration::from_secs(2 * 60);
/// Fetching a page or its images.
const FETCH_STEP_DEADLINE: Duration = Duration::from_secs(5 * 60);
/// Everything else calls an AI model (extracting from text or photos,
/// normalize ops, Edit with AI, resolving ingredient names), and those can be
/// slow.
const AI_STEP_DEADLINE: Duration = Duration::from_secs(15 * 60);

/// How long `step_name` may run.
pub fn step_deadline(step_name: &str) -> Duration {
    match step_name {
        ParseIngredientsStep::NAME
        | SaveRecipeStepMeta::NAME
        | LOAD_RECIPE_STEP
        | PROPOSE_NORMALIZE_STEP
        | APPLY_NORMALIZE_STEP => LOCAL_STEP_DEADLINE,
        FetchHtmlStep::NAME | FetchImagesStepMeta::NAME => FETCH_STEP_DEADLINE,
        _ => AI_STEP_DEADLINE,
    }
}

/// The error a step that ran past `deadline` fails with.
pub fn timeout_error(step_name: &str, deadline: Duration) -> String {
    format!(
        "Timed out: {step_name} was still running after {} and was stopped",
        describe(deadline)
    )
}

fn describe(deadline: Duration) -> String {
    let secs = deadline.as_secs();
    match secs {
        s if s >= 60 && s % 60 == 0 => match s / 60 {
            1 => "1 minute".to_string(),
            m => format!("{m} minutes"),
        },
        1 => "1 second".to_string(),
        s => format!("{s} seconds"),
    }
}

/// Run `step`, a step's execution, stopping it at `deadline`.
pub async fn run_with_deadline(
    step_name: &str,
    deadline: Duration,
    step: impl Future<Output = StepResult>,
) -> StepResult {
    match tokio::time::timeout(deadline, step).await {
        Ok(result) => result,
        Err(_) => StepResult {
            step_name: step_name.to_string(),
            success: false,
            output: serde_json::json!({ "timed_out": true }),
            error: Some(timeout_error(step_name, deadline)),
            duration_ms: deadline.as_millis() as u64,
            next_step: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ai_steps_get_longer_than_local_ones() {
        assert_eq!(step_deadline("save_recipe"), LOCAL_STEP_DEADLINE);
        assert_eq!(step_deadline("fetch_html"), FETCH_STEP_DEADLINE);
        for step in [
            "extract_recipe",
            "auto_tag",
            "edit_with_ai",
            "photo_extract",
        ] {
            assert_eq!(step_deadline(step), AI_STEP_DEADLINE, "{step}");
        }
    }

    #[tokio::test]
    async fn a_hung_step_fails_at_its_deadline() {
        let deadline = Duration::from_millis(20);
        let result =
            run_with_deadline("auto_tag", deadline, std::future::pending::<StepResult>()).await;
        assert!(!result.success);
        assert_eq!(result.error, Some(timeout_error("auto_tag", deadline)));
        assert_eq!(result.output["timed_out"], true);
    }

    #[tokio::test]
    async fn a_step_that_finishes_in_time_keeps_its_result() {
        let finished = StepResult {
            step_name: "auto_tag".to_string(),
            success: true,
            output: serde_json::json!({ "suggested_tags": [] }),
            error: None,
            duration_ms: 5,
            next_step: None,
        };
        let result =
            run_with_deadline("auto_tag", Duration::from_secs(60), async { finished }).await;
        assert!(result.success);
        assert_eq!(result.output["suggested_tags"], serde_json::json!([]));
    }

    #[test]
    fn deadlines_read_in_whole_units() {
        assert_eq!(
            timeout_error("auto_tag", Duration::from_secs(15 * 60)),
            "Timed out: auto_tag was still running after 15 minutes and was stopped"
        );
        assert_eq!(describe(Duration::from_secs(30)), "30 seconds");
    }
}
