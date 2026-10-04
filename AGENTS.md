When adding features or fixing bugs, handle both the web and iOS clients by default. Only scope work to one client after the user explicitly confirms the other client does not need the change.

When that means implementing the same pure logic on both clients (scaling, formatting, ordering, date math — anything with deterministic input/output pairs), follow doc/client-logic-sharing.md: push the logic to the server when it fits, otherwise pin both copies with shared test vectors in the same PR. Until the vector harness from that doc lands, at minimum mirror the unit tests on both sides and call out the duplication in the PR description.

We plan to never actually delete any data from the DB - everything will be soft-deletes.

When adding new API endpoints, remember to add end-to-end tests before you start using them in the UI.

# Pipeline

When you change extraction or parsing behavior, rerun `make pipeline` and commit the resulting data/ diffs. Those diffs are the point — they show the impact of your change and reviewers need to see them.

# Git

Only use commands like `git checkout` when you're in a workspace that you own (a Conductor workspace or Claude Code for Web). If you're in ~/code/ramekin, don't run git commands except read-only ones like status - I've probably made manual changes that you don't know about, and you've historically been overconfident about this kind of thing.

On Linux, do not attempt to run `make ios-test` or ask the user to install XcodeGen/Xcode. Submit the PR and use the existing macOS iOS GitHub Actions job to build and test iOS changes. Treat any failure there like any other test failure and fix it before finishing.

This repo uses the shared `agent-issues` workflow for local issues, claiming, and PR submission. See `doc/issues.md` for Ramekin-specific issue notes.
