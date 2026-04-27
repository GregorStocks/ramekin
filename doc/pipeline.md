# Pipeline Notes

`make pipeline` uses `data/test-urls.json` as the main corpus. That file can include URLs that were manually curated, not just URLs discovered by `generate-test-urls`.

## Snapshot allowlist

`data/pipeline-snapshot-urls.json` is the committed list of URLs that must produce end-of-pipeline snapshots in `data/pipeline-snapshots/`. Missing `extract_recipe` output for one of those URLs is a hard failure.

## Expected extract failures

`data/pipeline-expected-extract-failures.json` is the committed list of curated URLs that are intentionally kept in the pipeline corpus even though they are not expected to extract into recipes.

Use it for pages like editorial guides or reference articles that are useful to keep around for fetch/extract coverage, but that should not count as pipeline regressions in `data/extraction-report.md`.

Each entry is an object with:

- `url`: the exact URL in `data/test-urls.json`
- `reason`: why the URL is intentionally expected to fail `extract_recipe`
