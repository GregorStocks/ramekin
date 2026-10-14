# Cutting prod over from Postgres to SQLite

The server now runs on a SQLite file (`DATABASE_URL` is a path). This is the
one-time runbook for moving the existing Postgres data across. Background and
measurements: doc/sqlite-pilot-analysis.md.

Nothing here deletes data. Postgres stays intact (and dumped) as the rollback
path; soft-deleted rows move across like any other.

## What the importer does

`ramekin-server import-postgres <postgres-url>` copies the Postgres database
into the SQLite file named by `DATABASE_URL`, which must be new or empty:

1. Creates the schema (the same migrations the server runs at startup).
2. Reads every table in one repeatable-read Postgres snapshot and writes it in
   one SQLite transaction. If anything fails, the SQLite file is left with an
   empty schema; delete it and run again.
3. Converts types along the way: uuids to text, `uuid[]` photo ids and
   `jsonb` to compact JSON text, timestamps to UTC text, and fills the
   case-folded `username_key` / `name_key` columns.
4. Seeds the sync clock above the snapshot's `pg_snapshot_xmax`, so it starts
   above every cursor a client could hold. Every live recipe version, tag and
   shopping item gets a stamp above that, so each client's next normal delta
   sync re-delivers all of them. That matters because the ingredient search
   haystack changed from Postgres's jsonb text to compact JSON: re-delivery
   gives iOS the new text, so offline search keeps agreeing with the server
   with no client reset. Deletions keep their Postgres stamps.
5. Builds the search index (via the same triggers the server uses).
6. Checks that every table has the same row count on both sides, deleted rows
   included, and that every live recipe is indexed.

## Rehearse first

On a copy of prod, never prod itself:

```bash
pg_dump -Fc "postgres://ramekin:ramekin@localhost:54321/ramekin" > /tmp/ramekin-rehearsal.dump
createdb -h localhost -p 54321 -U ramekin ramekin_rehearsal
pg_restore -h localhost -p 54321 -U ramekin -d ramekin_rehearsal /tmp/ramekin-rehearsal.dump

make server-release-build
DATABASE_URL=/tmp/rehearsal.sqlite server/target/release/ramekin-server \
  import-postgres postgres://ramekin:ramekin@localhost:54321/ramekin_rehearsal
```

Then start a server on `/tmp/rehearsal.sqlite` (a spare `PORT`) and compare it
with the old server: the recipe list and a few recipes, search results, tags,
the shopping list and meal plans. A pre-cutover iOS install pointed at it
should receive every recipe on its next sync, and an offline search should
match a server search.

## Cutover

1. **Freeze writes.** `make serve-down` (stops the server, the scrape runner
   and the ingredient-name worker).
2. **Dump Postgres** for rollback, and keep the dump:
   `pg_dump -Fc "$OLD_DATABASE_URL" > ~/ramekin-postgres-final.dump`
3. **Import** into the new file (the old `dev.env` still names Postgres):

   ```bash
   make server-release-build
   DATABASE_URL="$PWD/.db/dev.sqlite" server/target/release/ramekin-server \
     import-postgres "$OLD_DATABASE_URL"
   ```

4. **Switch `DATABASE_URL`** in `dev.env` to that path (absolute).
5. **Start**: `make serve`. Sync the web app and iOS as usual; each client
   re-receives every recipe once.
6. **Check** an offline iOS search against the same server search, and the
   shopping list on both clients.
7. **Keep Postgres** (the container and the dump) read-only until you're
   satisfied. Nothing in the repo starts it any more: `docker start ramekin-db`
   brings it back for a look.

## Rollback

Check out the last pre-SQLite commit, point `DATABASE_URL` back at Postgres,
start it. Anything written on SQLite since the cutover stays only in the
SQLite file.

## Backups

The database is one file plus its `-wal`/`-shm` siblings. Never copy the file
while the server runs; use SQLite's online backup instead:

```bash
sqlite3 .db/dev.sqlite ".backup '/path/to/backup/ramekin-$(date +%F).sqlite'"
```

or `VACUUM INTO`, or run Litestream for continuous replication. Set one up
before cutover (tracked in `issues/`).
