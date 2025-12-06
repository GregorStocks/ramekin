Read through the makefile first. Strongly prefer to do things via existing makefile commands rather than by manually running docker, cargo, etc. If there isn't a makefile command for the thing that you want to do, consider making one.

The only dependencies to bring up the server should be docker and make. But we can also use uv and npx in the linter. Never use system Python or NPM because those are presumably always broken.

We plan to never actually delete any data from the DB - everything will be soft-deletes.

Always explicitly get signoff from the user before creating a new database migration, since we want to get them right the first time.

When adding new dependencies, make sure you're getting the latest version - you were trained several months ago so you probably don't know what the state of the art is.

When adding new API endpoints, remember to add end-to-end tests before you start using them in the UI.

Don't run git checkout unless you're confident you know what's happened since the last commit.

Never modify generated code (except for temporary testing), since your changes will get blown away.

Never bypass the linter with #noqa or equivalent. Never put a Python import anywhere other than the top of the file.
