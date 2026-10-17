# Database operations

The server runs on a SQLite file (`DATABASE_URL` is a path). Background on the
move off Postgres and its measurements: doc/sqlite-pilot-analysis.md.

## Stored files

Photo bytes and large step outputs (fetched pages, mostly) live beside the
database, not in it: `<DATABASE_URL>-files/`, one file per photo, thumbnail
or output, named by the SHA-256 of its contents (`ab/ab12…`). A file never
changes once written and is never deleted, a soft-deleted photo's included.
Keep the directory with the database file. A step output goes to a file when
its JSON is over 16 KB; its row then holds JSON null in `output` and the hash
in `output_hash`.

A database from before this layout moves its photo BLOBs out to files on the
next startup, 16 rows per write transaction, then VACUUMs and drops the
emptied columns. Large inline step outputs move the same way, 32 rows per
write transaction, then a VACUUM. Both are safe to interrupt: the next
startup carries on. (On a copy of prod on 2026-10-17, the output move took
672 fetched pages and 2 extractions out, 379 MB as 600 files after
duplicates, and the database went from 483 MB to 52 MB, in under 4 seconds.)
The store used to be `<DATABASE_URL>-photos/`; startup renames it.

## Backups

Set `RAMEKIN_BACKUP_DIR` in `dev.env`, pointing at a folder
that is synced or mounted off this machine. The server then writes a snapshot
there once an hour (on startup too, if the newest is more than an hour old)
and deletes snapshots older than 7 days. Without it, the server logs a warning at
startup and backs nothing up.

With each snapshot, the stored files the backup folder's `files/` lacks are
copied in before the snapshot gets its name, and the snapshot only gets it if
every file it names is there, so a snapshot you can see always has its files.
A file missing from the live store makes the snapshot fail (logged, retried
ten minutes later) rather than publish one that can't be fully restored.
Since stored files never change, that keeps `files/` complete for every
snapshot; it's never pruned. A backup folder from before fetched pages moved
to files has `photos/` instead; the first snapshot or restore renames it.

A snapshot is `ramekin-<UTC time>.sqlite`: SQLite's `VACUUM INTO`, a
consistent, compacted, self-contained copy taken while the server keeps
running. It's written under a temporary name and renamed into place, so a sync
tool never sees a partial file. Never copy the live database file itself; it
can be mid-write, and recent changes may still be in its `-wal` file.

Take one on demand (before an upgrade, say) with `make db-backup`, which writes
to `RAMEKIN_BACKUP_DIR` or `DIR=/somewhere`, stored files included.

## Restoring a backup

1. `make serve-down`.
2. `make db-restore SNAPSHOT=/path/to/ramekin-<time>.sqlite`.
3. `make serve`.

`db-restore` keeps the replaced database: it renames the file and its
`-wal`/`-shm` to `<database>.before-restore-<time>-<tag>`. Then it copies the
snapshot into place, and copies the stored files from the `files/` beside the
snapshot that `<database>-files/` lacks (all of them, on a new machine). If a
file the restored database names is still missing after that, it says how
many and exits with an error (the database is restored by then). The live
file directory isn't renamed: its files are a superset of what the snapshot
names.

It also moves the sync clock far past anything the replaced database handed
out, and restamps every live recipe, tag and shopping item above it. Clients
that synced after the snapshot hold cursors the restored clock would otherwise
reach again, and their later syncs would then silently skip the rolled-back
rows. With the restamp, every client's next sync re-delivers the restored
state, however long it waits to sync.

Anything created after the snapshot lingers in client caches until
`p4-full-resync-prune` lands.
