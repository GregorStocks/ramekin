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
   empty schema; delete it and run again. (Photo files already written can
   stay: the next run writes the same names.)
3. Converts types along the way: uuids to text, `uuid[]` photo ids and
   `jsonb` to compact JSON text, and timestamps to UTC text. The case-folded
   `username_key` / `name_key` columns fill themselves (generated columns).
   Photo and thumbnail bytes don't go in the database: each is written to a
   file named by its SHA-256 in `<DATABASE_URL>-photos/`, and the row keeps
   the hash (`data_hash`, `thumbnail_hash`).
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
the shopping list and meal plans, and that photos and thumbnails load. A
pre-cutover iOS install pointed at it
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

## Photo files

Photo bytes live beside the database, not in it: `<DATABASE_URL>-photos/`,
one file per photo or thumbnail, named by the SHA-256 of its contents
(`ab/ab12…`). A file never changes once written and is never deleted, a
soft-deleted photo's included. Keep the directory with the database file.

A database from before this layout moves its photo BLOBs out to files on the
next startup, 16 rows per write transaction, then drops the emptied columns
and VACUUMs. It's safe to interrupt: the next startup carries on.

## Backups

Set `RAMEKIN_BACKUP_DIR` in `dev.env` before cutover, pointing at a folder
that is synced or mounted off this machine. The server then writes a snapshot
there once a day (on startup too, if the newest is more than a day old) and
deletes snapshots older than 7 days. Without it, the server logs a warning at
startup and backs nothing up.

With each snapshot, the photo files the backup folder's `photos/` lacks are
copied in before the snapshot gets its name, so a snapshot you can see always
has its photos. Since photo files never change, that keeps `photos/` complete
for every snapshot; it's never pruned.

A snapshot is `ramekin-<UTC time>.sqlite`: SQLite's `VACUUM INTO`, a
consistent, compacted, self-contained copy taken while the server keeps
running. It's written under a temporary name and renamed into place, so a sync
tool never sees a partial file. Never copy the live database file itself; it
can be mid-write, and recent changes may still be in its `-wal` file.

Take one on demand (before an upgrade, say) with `make db-backup`, which writes
to `RAMEKIN_BACKUP_DIR` or `DIR=/somewhere`, photos included.

## Restoring a backup

1. `make serve-down`.
2. `make db-restore SNAPSHOT=/path/to/ramekin-<time>.sqlite`.
3. `make serve`.

`db-restore` keeps the replaced database: it renames the file and its
`-wal`/`-shm` to `<database>.before-restore-<time>-<tag>`. Then it copies the
snapshot into place, and copies the photo files from the `photos/` beside the
snapshot that `<database>-photos/` lacks (all of them, on a new machine). The
live photo directory isn't renamed: its files are a superset of what the
snapshot names.

It also moves the sync clock far past anything the replaced database handed
out, and restamps every live recipe, tag and shopping item above it. Clients
that synced after the snapshot hold cursors the restored clock would otherwise
reach again, and their later syncs would then silently skip the rolled-back
rows. With the restamp, every client's next sync re-delivers the restored
state, however long it waits to sync.

Anything created after the snapshot lingers in client caches until
`p4-full-resync-prune` lands.
