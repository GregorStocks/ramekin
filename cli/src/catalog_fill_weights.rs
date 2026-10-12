//! `make catalog-fill-weights`: ask the model once for the weights the
//! catalog can't give (a food with no density, or no weight for a counted
//! unit) across the committed ingredient corpora, and write the answers to
//! the catalog's `estimated_weights.json`, reviewed in a PR like any catalog
//! change. Calorie estimates then weigh those lines with no runtime AI call.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use ramekin_core::ai::{estimate_ingredient_weights, AiConfig, CachingAiClient};
use ramekin_core::catalog::{EstimatedWeight, EstimatedWeightsFile};

use crate::ingredient_catalog_audit::{read_json, weight_gaps};

const ESTIMATED_WEIGHTS_PATH: &str = "ramekin-core/src/catalog/data/estimated_weights.json";

/// Pairs per question.
const BATCH_SIZE: usize = 40;

pub async fn run(root: &Path, prod_recipes: Option<&Path>, learned: Option<&Path>) -> Result<()> {
    let gaps: Vec<(String, String)> = weight_gaps(root, prod_recipes, learned)?
        .into_iter()
        .map(|gap| (gap.food, gap.unit))
        .collect();
    if gaps.is_empty() {
        tracing::info!("The catalog has an estimate for every weight the corpora need");
        return Ok(());
    }
    let config = AiConfig::from_env()
        .context("AI is not configured (OPENROUTER_API_KEY in cli.env)")?
        .for_ingredients();
    let model = config.model.clone();
    let client = CachingAiClient::new(config);
    let path = root.join(ESTIMATED_WEIGHTS_PATH);
    let mut file: EstimatedWeightsFile = read_json(&path)?;
    tracing::info!("Estimating {} weights with {model}", gaps.len());

    // A rejected answer is asked again in halves, down to single pairs, as
    // the server's worker did; a pair that still fails is left out.
    let mut batches: Vec<Vec<(String, String)>> =
        gaps.chunks(BATCH_SIZE).rev().map(<[_]>::to_vec).collect();
    let (mut estimated, mut failed) = (0, Vec::new());
    while let Some(batch) = batches.pop() {
        match estimate_ingredient_weights(&client, &batch).await {
            Ok(result) => {
                for ((food, unit), grams) in result.weights {
                    let weight = EstimatedWeight {
                        grams,
                        model: model.clone(),
                    };
                    file.weights.entry(food).or_default().insert(unit, weight);
                    estimated += 1;
                }
                // Saved after every batch, so a provider failure loses nothing
                // (and the cache makes a rerun free up to that point).
                write(&path, &file)?;
            }
            Err(e) if e.is_answer_specific() && batch.len() > 1 => {
                let (first, second) = batch.split_at(batch.len() / 2);
                batches.push(second.to_vec());
                batches.push(first.to_vec());
            }
            Err(e) if e.is_answer_specific() => failed.push((batch[0].clone(), e)),
            Err(e) => {
                return Err(e).context("The provider failed; rerun to continue where this stopped")
            }
        }
    }
    tracing::info!("Wrote {estimated} estimated weights to {ESTIMATED_WEIGHTS_PATH}");
    for ((food, unit), e) in &failed {
        tracing::warn!("No estimate for {food:?} per {unit:?}: {e}");
    }
    Ok(())
}

fn write(path: &Path, file: &EstimatedWeightsFile) -> Result<()> {
    let json = serde_json::to_string_pretty(file)? + "\n";
    fs::write(path, json).with_context(|| format!("Failed to write {}", path.display()))
}
