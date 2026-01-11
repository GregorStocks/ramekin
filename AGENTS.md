Read through the makefile first. Always do things via existing makefile commands. Never manually run docker or cargo. If there isn't a makefile command for the thing that you want to do, ask if you should make one.

The only dependencies to bring up the server should be docker (for postgres), make, and cargo. We also use uv and npx for the linter and code generation. Never use system Python or NPM because those are presumably always broken.

We plan to never actually delete any data from the DB - everything will be soft-deletes.

Always explicitly get signoff from the user before creating a new database migration, since we want to get them right the first time.

When adding new dependencies, make sure you're getting the latest version - you were trained several months ago so you probably don't know what the state of the art is.

When adding new API endpoints, remember to add end-to-end tests before you start using them in the UI.

Never modify generated code (except for temporary testing), since your changes will get blown away.

Never bypass the linter with #noqa or equivalent. Never put a Python import anywhere other than the top of the file.

We do not need backwards compatibility. This does not exist in production. Do not keep unneeded code around for "backwards compatibility".

If a test is failing, you aren't done. There is no such thing as an unrelated test failure. Your extremely strong prior should be that you broke the test. Even if you didn't, you should fix it.

# Git

We use master, not main.

Only use commands like `git checkout` when you're in a workspace that you own (a Conductor workspace or Claude Code for Web). If you're in ~/code/ramekin, don't run git commands except read-only ones like status - I've probably made manual changes that you don't know about, and you've historically been overconfident about this kind of thing.

Never force-push without asking for permission. Merge master into your branch rather than rebasing so you don't need to force-push. Make new commits rather than amending. We want an honest Git history, not a clean one.

Always run `make lint` before creating a PR, and fix any lint errors it finds.

# Claude Web

If you are Claude for Web, first run `make setup-claude-web`. (If you're not sure, just go ahead and run it - it's a no-op if you aren't.)
