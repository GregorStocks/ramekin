Read through the makefile first. Always do things via existing makefile commands. Never manually run docker or cargo. If there isn't a makefile command for the thing that you want to do, ask if you should make one.

We use uv and npx for the linter and code generation. Never use system Python or NPM because those are presumably always broken.

When adding features or fixing bugs, handle both the web and iOS clients by default. Only scope work to one client after the user explicitly confirms the other client does not need the change.

When that means implementing the same pure logic on both clients (scaling, formatting, ordering, date math — anything with deterministic input/output pairs), follow doc/client-logic-sharing.md: push the logic to the server when it fits, otherwise pin both copies with shared test vectors in the same PR. Until the vector harness from that doc lands, at minimum mirror the unit tests on both sides and call out the duplication in the PR description.

We plan to never actually delete any data from the DB - everything will be soft-deletes.

When adding new dependencies, make sure you're getting the latest version - you were trained several months ago so you probably don't know what the state of the art is.

When adding new API endpoints, remember to add end-to-end tests before you start using them in the UI.

Never modify generated code (except for temporary testing), since your changes will get blown away.

Never bypass the linter with #noqa or equivalent. Never put a Python import anywhere other than the top of the file.

Use `tracing` (tracing::info!, tracing::warn!, etc.) for logging in Rust code, not println/eprintln.

If a test is failing, you aren't done. There is no such thing as an unrelated test failure. Your extremely strong prior should be that you broke the test. Even if you didn't, you should fix it.

Try very hard to avoid ever writing raw SQL. We should always use the regular Diesel DSL. If you're really sure that you need raw SQL, explicitly confirm with the user first.

We do not need backwards compatibility. This does not exist in production. Do not keep unneeded code around for "backwards compatibility". If you find yourself writing a comment about backwards compatibility or legacy support or anything like that, check with me because you are probably doing the wrong thing. (You've probably been trained to want to do this way more than makes sense - fight that instinct.)

Never fail gracefully, always fail fast. Check with me if you're not sure. (This is another training issue.)

# Pipeline

When you change extraction or parsing behavior, rerun `make pipeline` and commit the resulting data/ diffs. Those diffs are the point — they show the impact of your change and reviewers need to see them.

If you see changes in the working tree that you don't think belong in your PR, that is an extremely bad smell. Do not silently exclude files from your commit. Escalate to the user and ask — the changes are probably related to your work and you're wrong about them not belonging.

# Git

We use master, not main.

Only use commands like `git checkout` when you're in a workspace that you own (a Conductor workspace or Claude Code for Web). If you're in ~/code/ramekin, don't run git commands except read-only ones like status - I've probably made manual changes that you don't know about, and you've historically been overconfident about this kind of thing.

Always run `make lint` before creating a PR, and fix any lint errors it finds.

# Claude Web

If you are Claude for Web, first run `make setup-claude-web`. (If you're not sure, just go ahead and run it - it's a no-op if you aren't.)

This repo uses the shared `agent-issues` workflow for local issues, claiming, and PR submission. See `doc/issues.md` for Ramekin-specific issue notes.

You can put whatever docs you want in `docs/agent/` with the understanding that they're your own personal memory aids and not for humans.
