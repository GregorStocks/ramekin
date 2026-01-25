//! Database-backed step output store for the generic pipeline.

// Infrastructure for future generic pipeline integration.
// Currently unused while existing scraping code remains in use.
#![allow(dead_code)]

use std::error::Error;

use ramekin_core::pipeline::StepOutputStore;
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::db::DbPool;
use crate::models::{NewStepOutput, StepOutput};
use crate::schema::step_outputs;
use diesel::prelude::*;
use ramekin_core::BUILD_ID;

/// Database-backed output store for scrape jobs.
///
/// This implements the `StepOutputStore` trait, allowing the generic
/// pipeline executor to read and write step outputs to the database.
pub struct DbOutputStore<'a> {
    pool: &'a DbPool,
    job_id: Uuid,
}

impl<'a> DbOutputStore<'a> {
    /// Create a new DbOutputStore for a scrape job.
    pub fn new(pool: &'a DbPool, job_id: Uuid) -> Self {
        Self { pool, job_id }
    }
}

impl StepOutputStore for DbOutputStore<'_> {
    fn get_output(&self, step_name: &str) -> Option<JsonValue> {
        let mut conn = match self.pool.get() {
            Ok(conn) => conn,
            Err(e) => {
                tracing::error!("Failed to get DB connection: {}", e);
                return None;
            }
        };

        step_outputs::table
            .filter(step_outputs::scrape_job_id.eq(self.job_id))
            .filter(step_outputs::step_name.eq(step_name))
            .order(step_outputs::created_at.desc())
            .first::<StepOutput>(&mut conn)
            .optional()
            .ok()
            .flatten()
            .map(|so| so.output)
    }

    fn save_output(
        &mut self,
        step_name: &str,
        output: &JsonValue,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut conn = self.pool.get()?;

        let new_output = NewStepOutput {
            scrape_job_id: self.job_id,
            step_name: step_name.to_string(),
            build_id: BUILD_ID.to_string(),
            output: output.clone(),
        };

        diesel::insert_into(step_outputs::table)
            .values(&new_output)
            .execute(&mut conn)?;

        Ok(())
    }
}
