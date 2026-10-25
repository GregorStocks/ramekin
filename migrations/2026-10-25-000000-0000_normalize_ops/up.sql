-- Recipe operations are one registry of "normalize ops" now
-- (ramekin-core/src/pipeline/normalize_ops.rs). A version written by
-- normalize ops records which ones, as a JSON array of op ids, under the one
-- version source 'normalize'.
ALTER TABLE recipe_versions ADD COLUMN normalize_ops TEXT;

-- The ops a manual normalize job runs, as a JSON array of op ids. Such a job
-- proposes a version instead of saving one.
ALTER TABLE scrape_jobs ADD COLUMN normalize_ops TEXT;

UPDATE recipe_versions SET version_source = 'normalize', normalize_ops = '["normalize_title"]'
WHERE version_source = 'normalize_title';
UPDATE recipe_versions SET version_source = 'normalize', normalize_ops = '["generate_description"]'
WHERE version_source = 'generate_description';
UPDATE recipe_versions SET version_source = 'normalize', normalize_ops = '["auto_tag"]'
WHERE version_source = 'enrichment';

UPDATE scrape_jobs SET status = 'normalizing' WHERE status = 'enriching';

-- Step names: the AI steps lost their enrich_ prefix, and one apply_normalize
-- step replaced the three apply steps. An old apply step's row is not an
-- apply_normalize run (it applied one op), so it keeps a name of its own,
-- which apply_normalize reads to build on the versions those steps wrote.
UPDATE step_outputs SET step_name = 'normalize_title' WHERE step_name = 'enrich_normalize_title';
UPDATE step_outputs SET step_name = 'generate_description' WHERE step_name = 'enrich_generate_description';
UPDATE step_outputs SET step_name = 'auto_tag' WHERE step_name = 'enrich_auto_tag';
UPDATE step_outputs SET step_name = 'legacy_apply_normalize'
WHERE step_name IN ('apply_normalized_title', 'apply_generated_description', 'apply_auto_tags');
UPDATE scrape_jobs SET current_step = 'normalize_title' WHERE current_step = 'enrich_normalize_title';
UPDATE scrape_jobs SET current_step = 'generate_description' WHERE current_step = 'enrich_generate_description';
UPDATE scrape_jobs SET current_step = 'auto_tag' WHERE current_step = 'enrich_auto_tag';
-- A job stopped at an old apply step may still have had AI steps running, and
-- the concurrent phase treats every step before its resume point as done.
-- Resume at the first normalize step instead: steps that already succeeded
-- keep their outputs, and the rest run again.
UPDATE scrape_jobs SET current_step = 'normalize_title'
WHERE current_step IN ('apply_normalized_title', 'apply_generated_description', 'apply_auto_tags');
UPDATE scrape_jobs SET failed_at_step = 'normalize_title' WHERE failed_at_step = 'enrich_normalize_title';
UPDATE scrape_jobs SET failed_at_step = 'generate_description' WHERE failed_at_step = 'enrich_generate_description';
UPDATE scrape_jobs SET failed_at_step = 'auto_tag' WHERE failed_at_step = 'enrich_auto_tag';
UPDATE scrape_jobs SET failed_at_step = 'normalize_title'
WHERE failed_at_step IN ('apply_normalized_title', 'apply_generated_description', 'apply_auto_tags');

-- Re-extract finds a recipe's stored source through the jobs that worked on it.
CREATE INDEX scrape_jobs_recipe_id_idx ON scrape_jobs (recipe_id);
