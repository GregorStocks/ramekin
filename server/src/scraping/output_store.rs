//! Database-backed step output store for the server.

use std::error::Error;

use async_trait::async_trait;
use diesel::prelude::*;
use ramekin_core::pipeline::StepOutputStore;
use ramekin_core::BUILD_ID;
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::db::{run_blocking, DbPool};
use crate::models::{NewStepOutput, StepOutput};
use crate::schema::step_outputs;
use crate::scraping::status::step_summary;

/// Database-backed output store for server pipeline runs.
///
/// Stores step outputs in the step_outputs table, keyed by job_id and step_name.
pub struct DbOutputStore<'a> {
    pool: &'a DbPool,
    job_id: Uuid,
}

impl<'a> DbOutputStore<'a> {
    /// Create a new database output store for a job.
    pub fn new(pool: &'a DbPool, job_id: Uuid) -> Self {
        Self { pool, job_id }
    }
}

#[async_trait]
impl StepOutputStore for DbOutputStore<'_> {
    async fn get_output(&self, step_name: &str) -> Option<JsonValue> {
        let job_id = self.job_id;
        let step_name = step_name.to_string();

        run_blocking(self.pool, move |conn| {
            step_outputs::table
                .filter(step_outputs::scrape_job_id.eq(job_id))
                .filter(step_outputs::step_name.eq(step_name))
                .order(step_outputs::created_at.desc())
                .first::<StepOutput>(conn)
                .optional()
        })
        .await
        .ok()?
        .ok()?
        .map(|output| output.output)
    }

    async fn save_output(
        &mut self,
        step_name: &str,
        output: &JsonValue,
        duration_ms: i64,
        success: bool,
        error: Option<&str>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let new_output = NewStepOutput {
            scrape_job_id: self.job_id,
            step_name: step_name.to_string(),
            build_id: BUILD_ID.to_string(),
            output: output.clone(),
            duration_ms: Some(duration_ms),
            summary: step_summary(step_name, output),
            success,
            error: error.map(|s| s.to_string()),
        };

        run_blocking(self.pool, move |conn| {
            diesel::insert_into(step_outputs::table)
                .values(&new_output)
                .execute(conn)
        })
        .await
        .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?
        .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        Ok(())
    }
}
