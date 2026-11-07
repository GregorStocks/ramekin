//! A scrape job's status, stored as text in `scrape_jobs.status`.
//!
//! The migration `scrape_job_status_check` pins the stored values with a
//! CHECK, and the transition functions in `jobs` and `runner` only move a job
//! out of the statuses each transition allows (see [`JobStatus::ACTIVE`]).

use diesel::deserialize::{self, FromSql, FromSqlRow};
use diesel::expression::AsExpression;
use diesel::serialize::{self, IsNull, Output, ToSql};
use diesel::sql_types::Text;
use diesel::sqlite::{Sqlite, SqliteValue};

use super::photo_import::PHOTO_EXTRACT_STEP;
use super::plan::JobPlan;
use super::steps::FetchHtmlStep;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, AsExpression, FromSqlRow)]
#[diesel(sql_type = Text)]
pub enum JobStatus {
    /// Created, and not yet running a step (or waiting for a job slot).
    Pending,
    /// Reading the source: fetching the page, or extracting from photos.
    Scraping,
    /// Running the steps before the concurrent phase.
    Parsing,
    /// Running the concurrent phase. For an import or scrape the recipe is
    /// already saved and visible, and normalize ops may still update it; a
    /// manual normalize or edit job then proposes a version.
    Normalizing,
    /// Finished: `recipe_id` is set.
    Completed,
    /// Stopped on an error: `error_message` is set, and Retry may resume it.
    Failed,
}

impl JobStatus {
    /// Every status a job may still leave: the ones a step runs in.
    pub const ACTIVE: [JobStatus; 4] = [
        JobStatus::Pending,
        JobStatus::Scraping,
        JobStatus::Parsing,
        JobStatus::Normalizing,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            JobStatus::Pending => "pending",
            JobStatus::Scraping => "scraping",
            JobStatus::Parsing => "parsing",
            JobStatus::Normalizing => "normalizing",
            JobStatus::Completed => "completed",
            JobStatus::Failed => "failed",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        [Self::Completed, Self::Failed]
            .into_iter()
            .chain(Self::ACTIVE)
            .find(|status| status.as_str() == s)
    }

    /// Whether the job has stopped: nothing runs it any more.
    pub const fn is_terminal(self) -> bool {
        matches!(self, JobStatus::Completed | JobStatus::Failed)
    }

    /// The status a job has while it runs `step` of `plan`. Every place that
    /// starts a step takes its status from here.
    pub fn for_step(plan: &JobPlan, step: &str) -> Self {
        if step == FetchHtmlStep::NAME || step == PHOTO_EXTRACT_STEP {
            JobStatus::Scraping
        } else if plan.is_concurrent(step) {
            JobStatus::Normalizing
        } else {
            JobStatus::Parsing
        }
    }
}

impl std::fmt::Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromSql<Text, Sqlite> for JobStatus {
    fn from_sql(value: SqliteValue<'_, '_, '_>) -> deserialize::Result<Self> {
        let text = <String as FromSql<Text, Sqlite>>::from_sql(value)?;
        Self::parse(&text).ok_or_else(|| format!("unknown scrape job status {text:?}").into())
    }
}

impl ToSql<Text, Sqlite> for JobStatus {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Sqlite>) -> serialize::Result {
        out.set_value(self.as_str());
        Ok(IsNull::No)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statuses_round_trip_through_their_text() {
        for status in JobStatus::ACTIVE
            .into_iter()
            .chain([JobStatus::Completed, JobStatus::Failed])
        {
            assert_eq!(JobStatus::parse(status.as_str()), Some(status));
        }
        assert_eq!(JobStatus::parse("enriching"), None);
    }

    #[test]
    fn only_completed_and_failed_are_terminal() {
        assert!(JobStatus::Completed.is_terminal());
        assert!(JobStatus::Failed.is_terminal());
        assert!(JobStatus::ACTIVE.iter().all(|s| !s.is_terminal()));
    }

    #[test]
    fn steps_map_to_one_status_each() {
        let import = JobPlan::import();
        assert_eq!(
            JobStatus::for_step(&import, "fetch_html"),
            JobStatus::Scraping
        );
        assert_eq!(
            JobStatus::for_step(&import, PHOTO_EXTRACT_STEP),
            JobStatus::Scraping
        );
        for step in ["extract_recipe", "parse_ingredients", "save_recipe"] {
            assert_eq!(JobStatus::for_step(&import, step), JobStatus::Parsing);
        }
        for step in ["resolve_ingredient_names", "auto_tag", "apply_normalize"] {
            assert_eq!(JobStatus::for_step(&import, step), JobStatus::Normalizing);
        }
        let edit = JobPlan::edit();
        assert_eq!(
            JobStatus::for_step(&edit, "load_recipe"),
            JobStatus::Parsing
        );
        assert_eq!(
            JobStatus::for_step(&edit, "edit_with_ai"),
            JobStatus::Normalizing
        );
    }
}
