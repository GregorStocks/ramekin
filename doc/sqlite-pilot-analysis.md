# SQLite pilot: pre-analysis

Step 1 of the "make the database layer generic and pilot SQLite" task: what
moving off Postgres would take, what it would do to performance, and a
go/no-go recommendation. Nothing here changes code; step 2 waits on the
owner's decision (`issues/blocked-database-generic-sqlite-step2.json5`).

## Recommendation: go, as a cutover rather than a permanent second backend

Performance is not a reason to stay on Postgres. At realistic single-user sizes,
SQLite (WAL mode, FTS5 for search) matched or beat Postgres on every hot path
measured, often by a wide margin. The one real cost is that writes serialize.
The longest write measured, rewriting every current recipe at 5k recipes, took
under 300 ms on SQLite. A small write behind it waits that long.

The real cost is engineering effort, roughly two to four focused PRs (see
[Effort](#effort)). Most of it is in four areas: the search filter, the sync
change cursor, the `uuid[]` / `jsonb` / `citext` columns, and the schema types
Diesel generates per backend. Because the schema types differ between
backends, keeping both backends forever would mean two `schema.rs`
files, per-backend SQL fragments, and running the e2e suite twice on every PR,
for an app with one user. Recommendation: build SQLite support
behind a feature/backend switch, run the e2e suite against both only until
SQLite passes, cut prod over, then delete the Postgres path.

Two caveats the owner should weigh:

- **Hosting.** If the motivation is running the whole app inside Cloudflare
  (`blocked-ramekin-mcp-chatgpt`), note that Cloudflare's SQLite offerings (D1,
  Durable Object storage) are reached through Workers bindings, not a local
  file Diesel can open. A self-hosted server on a local SQLite file, which
  this analysis measures, is a different deployment from "SQLite in
  Cloudflare". That option needs its own investigation before this one is
  chosen for it.
- **Operational tooling.** Postgres currently gives you `psql`, `pg_dump`,
  and the ability to inspect a live DB alongside the server. SQLite gives you
  `sqlite3`, `.backup`/`VACUUM INTO`, or Litestream for continuous backup.
  That is simpler, but it is different, and backups have to be set up before
  cutover.

## Inventory of Postgres-specific features

Cost: S = mechanical, M = needs design or a few files, L = cross-cutting.

| Feature | Where | SQLite equivalent | Cost |
|---|---|---|---|
| `pg_current_xact_id()` / `pg_snapshot_xmin()` sync cursors (`current_change_xid()`, `change_xid_watermark()`), `stamp_change_xid()` trigger | migrations `2026-07-12…sync_change_xid`, `2026-10-11…shopping_list_change_xid`; `server/src/raw_sql.rs`; `api/recipes/sync.rs`, `api/shopping_list/sync.rs`; hand-stamped in `tags.rs`, `api/tags/*`, `api/recipes/delete.rs` | A one-row `sync_clock` counter bumped inside each write transaction. SQLite has a single writer, so commit order equals counter order and the watermark can simply be the current value. The xmin race the Postgres design guards against can't happen. Triggers become SQLite `AFTER UPDATE` triggers that read the counter. This also fixes `p3-sync-cursor-after-restore`, because the counter is data and is dumped with the DB. Coordinate with `blocked-one-sync-mechanism`. | M |
| Trigram + unaccent search: `pg_trgm`, `unaccent`, `f_unaccent()`, five GIN expression indexes, `ILIKE` | `api/recipes/list.rs` text and source filters; migration `2026-04-16…unaccent_search` | FTS5 table with the plain `trigram` tokenizer over the current versions, kept in sync on version insert. Columns are pre-normalized in Rust with `ramekin_core::search::normalize_for_search`, and query tokens go through the same function. FTS5's `remove_diacritics` can't be used on its own: it strips a narrower set than the search normalization contract, so `oeufs` wouldn't find `Œufs`, and `ß`, `æ`, `ø`, ligatures and fractions would also differ. Tokens shorter than 3 characters can't use a trigram index, so they fall back to a scan with a `normalize_for_search` UDF registered on each connection. That UDF also covers the `source:` filter. The contract asset (`shared-test-vectors/search-normalization.json`) is currently regenerated from Postgres's `unaccent`/`lower()` by `scripts/generate-search-normalization.sh`. On SQLite it becomes the frozen source of truth. `tests/test_search_normalization_contract.py` has to change from "matches the live server" to "the server's search obeys the asset". In the benchmark, row sets matched `f_unaccent(..) ILIKE` for the four test queries, but those queries didn't exercise the ligature expansions. Step 2 must run the contract vectors, such as `tests/test_recipes.py`'s `Œufs` case, against SQLite. | M |
| `recipe_versions.ingredients::text` as the sync search haystack (`ingredient_match_text`) | `raw_sql::ingredients_text()`, recipe sync, iOS local search | Ingredients become a JSON `TEXT` column, so the haystack is that text. Postgres's jsonb serialization (`{"a": 1}` with spaces) differs from compact JSON. iOS matches against whatever string the server sends, so this only changes the served bytes. A full resync re-delivers them, so it's not a client change. | S |
| `jsonb` columns (`recipe_versions.ingredients`, `step_outputs.output`) | models, scraping output store | Stored as `TEXT` JSON. No jsonb operators are used in queries (only `::text`), so this is a type swap. Diesel 2.3's SQLite backend supports `Json`/`Jsonb` types. | S |
| `uuid` primary/foreign keys, `gen_random_uuid()` defaults | every table | Diesel's SQLite backend has no `Uuid` SQL type. Store as `TEXT` (or 16-byte `BLOB`) with a small `FromSql`/`ToSql` newtype, and generate ids in Rust (most inserts already do). | M (wide but mechanical) |
| `uuid[]` `recipe_versions.photo_ids`; `= ANY(photo_ids)`, `cardinality()` | `raw_sql.rs` photo filters, `api/recipes/list.rs` has-photos filter, photo loading | Either JSON array `TEXT` with `json_each` / `json_array_length`, or a `recipe_version_photos` junction table that preserves order. The junction table is cleaner and needs a data migration either way. | M |
| `array_agg(... ORDER BY)` tags subquery | `raw_sql::tags_subquery()` | `json_group_array` over an ordered subquery, deserialized as JSON. Compute it only for the page after `LIMIT`. SQLite evaluates result-column subqueries before sorting, which made an unpaged version ~3× slower in the benchmark. | S |
| `citext` (`user_tags.name`) | tags | `COLLATE NOCASE` only folds ASCII. Store a normalized `name_key` column (Unicode lowercase in Rust) with the unique index on `(user_id, name_key)`. | S |
| `timestamptz` | every table | Diesel's SQLite `TimestamptzSqlite` (chrono) stores ISO-8601 text that sorts correctly. `diesel::dsl::now` works. The sync code's `day_start_utc` comparisons keep working as long as every timestamp is written in UTC. | S |
| `bytea` photo data and thumbnails | `photos.data`, `photo_thumbnails` | `BLOB`. The DB file grows with photos. That's fine for SQLite, but backups become whole-file copies. Moving photos to files on disk is optional and separate. | S |
| `SELECT … FOR UPDATE` | `ingredient_names.rs:690`, `scraping/steps/save_recipe.rs:333` | Not needed: `BEGIN IMMEDIATE` takes the write lock up front. Diesel: `conn.immediate_transaction(..)`. | S |
| `ON CONFLICT ON CONSTRAINT uq_shopping_list_client_id` | `api/shopping_list/create.rs`, `sync.rs` | `on_conflict((user_id, client_id))` by columns (Diesel's SQLite backend has no `on_constraint`). Plain `on_conflict_do_nothing` and column-target upserts in `tags.rs`, `recipes.rs`, `auth/db.rs`, `get_thumbnail.rs` work as-is. | S |
| `REPEATABLE READ READ ONLY` sync transaction | `api/recipes/sync.rs` | A plain `BEGIN` read transaction in WAL mode is already a stable snapshot. | S |
| Partial indexes (`WHERE deleted_at IS NULL`) | several migrations | Supported natively. | S |
| `random()` sort | `api/recipes/list.rs` | `random()` exists (returns an integer, which is still a valid sort key). | S |
| `RETURNING` | many inserts/updates | Supported since SQLite 3.35. Enable Diesel's `returning_clauses_for_sqlite_3_35` feature. | S |
| 51 Postgres SQL migrations | `migrations/` | Squash into one fresh SQLite baseline migration for the new backend. Postgres migration history doesn't need to replay on SQLite. | M |
| `PgConnection` in signatures | 44 references across 10 files (`db.rs`, `data_migrations.rs`, `ingredient_names.rs`, scraping, …) | A `DbConnection` alias or Diesel `MultiConnection` during the transition, then just `SqliteConnection`. | S |

Not used, so nothing to port: `LISTEN`/`NOTIFY` (the ingredient-name worker
wakes on an in-process `tokio::sync::Notify`), advisory locks, `DISTINCT ON`,
jsonb operators, intervals, full-text `tsvector`, and Postgres enums.

## Concurrency model

Today an r2d2 pool (default 10 connections) serves requests through
`db::run_blocking`, while background work writes concurrently: the scrape
runner (`scraping/runner.rs`, `save_recipe` and its follow-up steps), the
ingredient-name worker (`ingredient_names.rs`), photo import, the dimension
backfill, and startup's `queue_stored` / `reconcile_categories` /
`data_migrations::run_pending`.

On SQLite in WAL mode, readers never block and never block the writer, but
only one write transaction runs at a time. Requirements for the port:

- `PRAGMA journal_mode=WAL; synchronous=NORMAL; foreign_keys=ON; busy_timeout=…`
  on every pooled connection (r2d2 `CustomizeConnection`).
- **Serialize writes in the app** instead of relying on `busy_timeout` alone:
  either a single dedicated writer connection or a write mutex around
  `immediate_transaction`. In the benchmark, two writers contending through
  SQLite's busy handler alone left one writer starved for the entire 5 s
  window once. With an app-level mutex, worst-case write latency stayed under
  ~150 ms.
- Keep write transactions short. Existing writes are already per-recipe or
  per-batch. The largest are data migrations (one transaction over every
  version) and `queue_stored`. Both run at startup before the server accepts
  requests, so they block nothing.
- AI calls and HTTP fetches already happen outside transactions in the scrape
  steps. That rule has to keep holding, because a network call inside a
  SQLite write transaction would stall every other writer.

## Performance measurements

`scripts/sqlite-pilot-bench.py` loads the 445 recipes in
`data/pipeline-snapshots/` (real titles, descriptions, ingredients and
instructions), replicated to N current recipes with 3 versions each and 2 tags
per version, for one user. It loads them into a Postgres 16 database with the
production search indexes and into a SQLite 3.50 file (WAL, FTS5 trigram), then
times each workload through Python drivers (psycopg 3, stdlib `sqlite3`).
Postgres columns are cast to text in the queries so driver-side
jsonb/uuid parsing doesn't count against it.

Prod data sizes weren't available. The two scales bracket a plausible
personal collection: 1k recipes is likely close, and 5k is a generous upper
bound. The machine was busy (load average ~15) during the runs, so treat
single numbers as ±30 %. The orders of magnitude were stable across repeated
runs.

Median latency (ms):

| Workload | PG 1k | SQLite 1k | PG 5k | SQLite 5k |
|---|---:|---:|---:|---:|
| List page (50, newest first, tags) + total count | 3.2 | 3.0 | 13 | 20 |
| Search "chicken" (220 / 1110 rows) | 74 | 3.1 | 501 | 31 |
| Search "garlic butter" (193 / 985 rows) | 95 | 1.7 | 649 | 43 |
| Search "brulee", diacritic-insensitive (3 / 12 rows) | 0.7 | 0.3 | 4.1 | 5.2 |
| Search with no match | 0.2 | 0.3 | 1.1 | 4.2 |
| Full recipe sync sweep (all pages of 200) | 65 | 6.5 | 733 | 177 |
| Single recipe save (new version + tags + pointer) | 2.9 | 0.4 | 9.9 | 1.5 |
| Bulk rewrite of every current version (one txn) | 1415 | 75 | 7477 | 284 |

Mixed load, 5 s with 4 reader threads (list + search) and 2 writer threads
saving recipes back-to-back, at 5k recipes:

| | reads p50 / p99 | writes p50 / p99 |
|---|---|---|
| Postgres | 72 / 1218 ms | 11 / 102 ms |
| SQLite, busy_timeout only | 145 / 391 ms | 3.0 / 62 ms |
| SQLite, app write mutex | 149 / 374 ms | 11 / 67 ms |

A single save issued while the bulk rewrite runs waited 18 ms on Postgres and
235 ms on SQLite, roughly the length of the SQLite bulk rewrite. Postgres saves
don't wait on other rows' writers, but its bulk rewrite took 26× longer because
it maintains five GIN trigram indexes.

What the numbers say:

- **Search is the biggest difference, and it favors SQLite.** The Postgres
  trigram indexes cover every version, not just current ones. For a common
  word, the index matches thousands of rows that then need `f_unaccent`
  rechecks over the whole instructions and ingredients text. `EXPLAIN ANALYZE`
  at 5k recipes shows ~300 ms of that server-side, so the cost is real and not
  driver overhead. An FTS5 index over current versions only is far cheaper.
  (Indexing only current versions would also help on Postgres, as a separate
  improvement.)
- **Sync and list are in-process lookups on SQLite.** There's no socket round
  trip or result serialization, which dominates the full sync sweep.
- **The scan fallback for <3-char tokens is the slow path.** With a Python
  UDF over five fields, the scan took 0.15 s at 1k and 1.6 s at 5k. A Rust UDF
  should be several times faster, and the path only triggers for queries made
  entirely of 1–2 character tokens. If that's a concern, the fallback can scan
  only `title`.
- **Serialized writes didn't matter at this scale.** Worst-case write latency
  under contention was in the tens of milliseconds, except while a whole-table
  rewrite holds the lock.

## Data migration plan (outline, for step 2)

1. Freeze writes: stop the server and the scrape/ingredient workers.
2. `pg_dump` for a rollback copy, kept indefinitely. No data is deleted; soft-
   deleted rows and their `deleted_at` / `deleted_xid` move over unchanged.
3. Create the SQLite baseline schema, then copy table by table through a Rust
   or Python exporter. The exporter converts uuids to text, `uuid[]` to the
   junction table or JSON, jsonb to compact JSON text, citext to text plus
   `name_key`, and timestamps to UTC ISO-8601.
4. Seed `sync_clock` from `pg_snapshot_xmax(pg_current_snapshot())` captured
   at the freeze, not from the highest stored stamp. A client's persisted
   cursor is a past `pg_snapshot_xmin()` watermark, which can be above every
   stored `change_xid` if the DB was idle after that sync. Seeding below it
   would make the first post-cutover writes invisible to that client forever.
   (Rewriting all stamps and restarting the clock low is not a substitute. It
   only works if the server also rejects pre-cutover cursors and forces a
   cursorless full sync.)
5. Build the FTS5 index from current versions.
6. Verify row counts per table, including soft-deleted rows, and spot-check
   the API responses against the Postgres instance for a sample of recipes and
   the shopping list.
7. Switch `DATABASE_URL`, start the server, run a client full sync on web and
   iOS, then keep Postgres around read-only until the owner is satisfied.

## Effort

Rough sequence for step 2:

1. Connection abstraction, SQLite pool with pragmas and write serialization,
   and the baseline SQLite migration. Type newtypes for uuid and JSON.
2. Port the four non-mechanical areas: the search filter (FTS5 plus the
   fallback UDF), sync cursors, photo-id arrays, and tag names. Run the
   e2e suite against SQLite in CI next to Postgres.
3. Data exporter and the cutover runbook, rehearsed on a copy of prod.
4. After cutover, delete the Postgres backend, its migrations and the
   dual-backend CI job.

## Reproducing

```bash
psql -h localhost -p 54321 -U ramekin -d postgres -c 'CREATE DATABASE ramekin_sqlite_bench'
.venv/bin/python scripts/sqlite-pilot-bench.py data/pipeline-snapshots \
  postgresql://ramekin:ramekin@localhost:54321/ramekin_sqlite_bench /tmp/bench.sqlite 1000
```
