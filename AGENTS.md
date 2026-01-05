Read through the makefile first. Always do things via existing makefile commands. Never manually run docker or cargo. If there isn't a makefile command for the thing that you want to do, ask if you should make one.

The only dependencies to bring up the server should be docker and make. But we can also use uv and npx in the linter. Never use system Python or NPM because those are presumably always broken.

We plan to never actually delete any data from the DB - everything will be soft-deletes.

Always explicitly get signoff from the user before creating a new database migration, since we want to get them right the first time.

When adding new dependencies, make sure you're getting the latest version - you were trained several months ago so you probably don't know what the state of the art is.

When adding new API endpoints, remember to add end-to-end tests before you start using them in the UI.

Never run git commands unless `CLAUDE_CODE_REMOTE=true` (web/CI environment). We use master, not main.

Never modify generated code (except for temporary testing), since your changes will get blown away.

Never bypass the linter with #noqa or equivalent. Never put a Python import anywhere other than the top of the file.

We do not need backwards compatibility. This does not exist in production. Do not keep unneeded code around for "backwards compatibility".

If a test is failing, you aren't done. There is no such thing as an unrelated test failure. Your extremely strong prior should be that you broke the test. Even if you didn't, you should fix it.

