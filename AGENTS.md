Read through the makefile first. Strongly prefer to do things via existing makefile commands rather than by manually running docker, cargo, etc. If there isn't a makefile command for the thing that you want to do, consider making one.

The only dependencies to bring up the server should be docker and make. But we can also use uv and npx in the linter. Never use system Python or NPM because those are presumably always broken.

We plan to never actually delete any data from the DB - everything will be soft-deletes.

Always explicitly get signoff from the user before creating a new database migration, since we want to get them right the first time.
