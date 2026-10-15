# What the server does at startup

Everything `ramekin-server` (no subcommand) does between launch and accepting
requests, in order, with why each step exists, what it costs, and what a
failure does. Code: `main()` in `server/src/main.rs` and `create_pool` /
`open_migrated` in `server/src/db.rs`.

## How it's run

`make serve` runs `scripts/serve-release.sh` under process-compose with
`restart: on_failure`, and that script runs the binary under `systemfd`. So the
port is already bound while the server starts: requests wait rather than being
refused, and a slow startup shows up as slow first requests. A panic in a
blocking step below exits the process and process-compose starts it again, so
a failure that persists becomes a restart loop and keeps the API down until
someone fixes it. A panic inside a background task ends only that task,
since nothing joins it. Those tasks are steps 5, 6 and 11, plus the job
runners step 8 starts. The server keeps serving without that task until the
next restart; a job whose runner panicked stays stuck until then.

## Measured cost

Release build, run against a copy of the prod database on 2026-10-14 (1,000
live recipes, 10 live shopping items, 866 learned ingredient names, 1,599
photos, no interrupted jobs). From launch to listening took 340-510 ms over
three runs. One run (run 3) broke down as:

| Step | Time |
| --- | --- |
| Open database, schema and data migrations | ~15 ms |
| `reconcile_categories` (almost all of it the first load of the ingredient catalog, which some later step would pay for otherwise) | ~160 ms |
| `resume_interrupted_jobs` | ~1 ms |
| `sync_with_catalog` | ~50 ms |
| `queue_stored` (queued nothing) | ~270 ms |
| Start the name worker, build the router, bind | ~10 ms |

Startup costs about half a second, and `queue_stored` is the only step whose
cost grows with the recipe collection.

## The steps

### 1. Telemetry (`init_telemetry`)

- **What:** Sets up console logging. If `OTEL_EXPORTER_OTLP_ENDPOINT` is set,
  it also tries a TCP connection to the endpoint (100 ms timeout) and exports
  traces and logs there if the connection succeeds.
- **Cost:** Only when the endpoint is set. The connection attempt is capped
  at 100 ms, but the hostname is first resolved with a synchronous
  `to_socket_addrs()` that has no time limit, so slow DNS stalls startup.
- **On failure:** If the collector can't be reached, the server logs to the
  console only. If the exporter can't be built, the server panics.
- **Must block serving:** Yes. Logging has to be set up before anything else.
- **Recommendation:** Keep.

`DATABASE_URL` is read right after this. If it's missing or not valid
Unicode, the server panics before touching the database. It's required, so
that's correct. Subcommands (`backup`, `restore`, `import-postgres`) return
at this point, and none of the steps below run for them.

### 2. Schema migrations (`open_migrated`, `run_pending_migrations`)

- **What:** Creates the database directory if it's missing, opens the file,
  applies the connection pragmas, turns on WAL, and runs any pending Diesel
  migrations. After the SQLite cutover (aa5cd81cd) the migration history
  starts from a single baseline.
- **Cost:** One query when nothing is pending. A migration's own cost
  otherwise.
- **On failure:** Panics.
- **Must block serving:** Yes. The code assumes the current schema.
- **Recommendation:** Keep.

### 3. Data migrations (`data_migrations::run_pending`)

- **What:** Runs Rust migrations that need server logic, such as the
  ingredient parser or the catalog, or that rewrite a big table. Each one
  runs once, either in the same transaction as the row that records it or,
  for a batched one, one short transaction per batch. Added in 61e3efdb5.
- **Today:** The list holds `compress_step_outputs`, which zstd-compresses
  step outputs stored as plain JSON text, 100 rows per transaction. When
  this issue was filed, the list held `strip_materialized_grams` and
  `promote_section_headers`, which rewrote every recipe. Both had already
  run in prod (their rows are in `data_migrations`), and the SQLite move
  (aa5cd81cd) deleted them.
- **Cost:** One `SELECT` on `data_migrations` once everything has run.
- **On failure:** Startup aborts, which is what a migration should do.
- **Must block serving:** Yes, whenever there's a migration to run.
- **Recommendation:** Keep the runner. Keep deleting each migration once it
  has run in prod, as the module comment already says to.

### 4. Connection pool (`create_pool`)

- **What:** Builds an r2d2 pool of up to 32 connections, with 4 kept open.
  Every connection gets the same pragmas and registers the
  `normalize_for_search` and `fold_case` SQL functions.
- **Cost:** Constant.
- **On failure:** Panics.
- **Must block serving:** Yes. Every later step uses the pool.
- **Recommendation:** Keep.

### 5. Photo dimension backfill (`photos::spawn_dimension_backfill`), in the background

- **What:** Decodes each live photo that has neither dimensions nor a
  `file_size`, then stores its dimensions and size. Added in #387 (49e3608db,
  2026-04) for the `photo_dim:` / `photo_size:` filters on photos stored
  before those columns existed.
- **Today:** Nothing is left for it to do. The #387 migration filled in
  `file_size` for every existing photo. Every code path that writes a photo
  (upload, scrape, generated photo) sets `file_size`. The Postgres import
  carried `file_size` over. In the prod copy, 0 photos match. What remains is
  one `COUNT(*)` over `photos` on every start.
- **On failure:** Logs a warning.
- **Must block serving:** No, and it doesn't.
- **Recommendation:** Delete it. The work is finished.

### 6. Backup scheduler (`backup::spawn_scheduler`), in the background

- **What:** If `RAMEKIN_BACKUP_DIR` is set, takes a snapshot right away when
  the newest one is more than a day old, then one a day after that, and
  prunes old snapshots. Added in 8fa32b720; see `doc/sqlite-cutover.md`,
  "Backups".
- **Cost:** Blocks only for `create_dir_all`. The background task scans the
  directory for the newest snapshot. If it's missing or more than a day old,
  the task runs `VACUUM INTO` right away, which reads the whole database
  (about 1 GB today) while the server serves.
- **On failure:** If the directory can't be created, the server panics. A
  failed snapshot logs an error and retries.
- **Must block serving:** No, and it doesn't.
- **Recommendation:** Keep.

### 7. `reconcile_categories`, blocks serving

- **What:** Recomputes the category of every live shopping-list item and
  writes only the items whose category changed. Those writes give the items
  new sync stamps, so clients pick up the changes. This handles items whose
  recategorization a crash interrupted, and items a catalog deploy
  reclassifies. Added in 288838029.
- **Why first:** List reads return the stored category. This step also has to
  read learned answers before anything can change them. Steps 9 and 11 can
  change them.
- **Cost:** Linear in live shopping items, which number in the tens. Runs as
  one write transaction. Its ~160 ms is almost entirely the first load of
  the catalog's embedded JSON, which a later step would otherwise pay.
- **On failure:** Panics.
- **Recommendation:** Keep, blocking.

### 8. `resume_interrupted_jobs`, blocks serving

- **What:** Restarts scrape and import jobs that were in progress when the
  previous process stopped. It marks photo-extraction jobs failed instead,
  because their upload is gone. Starts each job as a background task, with at
  most 8 running at once (`MAX_RUNNING_JOBS`).
- **Why blocking:** Before 8f743c1f7 this scan ran in a detached task. A job
  created by an early request could then be picked up a second time, and two
  runners would save the same recipe.
- **Cost:** One query over `scrape_jobs`, filtered by status, plus one
  blocking write per interrupted photo job it marks failed. The jobs it
  restarts run in the background.
- **On failure:** Logs an error and keeps going, so interrupted jobs stay
  stuck until the next restart.
- **Recommendation:** Keep.

### 9. `ingredient_names::sync_with_catalog`, blocks serving

- **What:** Puts learned ingredient names back in the queue when the
  committed catalog no longer accepts their stored answer: the key no longer
  names a single entry, or the catalog now calls the name ambiguous. Then it
  recategorizes the shopping items that use those names. Added in 807eb9781.
  cbe14ffa6 narrowed it to these two cases, removing the re-asks after a
  model change (`reask_other_models`) and the harvested state.
- **Cost:** One in-memory catalog lookup per resolved name, with no
  `candidates()` scan any more. Linear in resolved names (863): ~50 ms with
  nothing to change. After a catalog deploy that invalidates answers, it
  also updates those rows, scans every live shopping item, and rewrites the
  categories that changed. That adds cost in proportion to the shopping list
  and the number of invalidated names.
- **On failure:** Panics.
- **Must block serving:** Yes. Reads apply stored answers through
  `load_learned`, and a stored `not_food` answer still applies to a name the
  new catalog calls ambiguous (`resolve_learned` returns `NotFood` whatever
  the catalog said). Until this runs, calorie estimates and categories would
  use answers the deployed catalog rules out. It must also finish before the
  name worker resolves anything.
- **Recommendation:** Keep, blocking.

### 10. `ingredient_names::queue_stored`, blocks serving

- **What:** Parses the ingredients of every live recipe's current version
  and every live shopping item, and queues each name the catalog doesn't
  know. Saves already queue their own names. This step catches names that a
  catalog deploy stopped recognizing. Made blocking and fatal in 323a6b187.
- **Cost:** Linear in recipes × ingredients. The largest step at about
  270 ms for 1,000 recipes: a full scan of current versions, a JSON parse
  per recipe, and a catalog lookup per ingredient. It queues nothing unless
  the catalog changed.
- **On failure:** Panics.
- **Must block serving:** Not strictly. Reads use only resolved answers, so
  a name this hasn't queued yet reads the same as a queued one. The one
  difference: reads won't report that a fresher answer is coming
  (`any_pending`).
- **Recommendation:** Keep for now (see below).

### 11. Name worker (`ingredient_names::spawn_worker`), in the background

- **What:** Resolves pending names through the LLM in batches, starting
  with whatever was pending at startup, and backs off when the provider
  fails.
- **Cost:** Doesn't block. Right after startup it drains any names left
  pending, one LLM batch of 40 at a time, which grows with the number of
  pending names (0 in the prod copy).
- **Recommendation:** Keep.

### 12. Router and listener

- **What:** Builds the router, takes the listener from `systemfd` or binds
  `PORT`, and serves.
- **Cost:** Constant.
- **On failure:** Run directly, the server panics if `PORT` is missing or
  invalid, or if it can't bind the socket. The panic comes only after steps
  2–10 have run. Under `make serve` that can't happen:
  `serve-release.sh` rejects a missing `PORT`, and `systemfd` fails on a
  malformed or busy port before it launches the server. It then hands over
  a socket it has already bound.
- **Recommendation:** Keep.

## Summary

| Step | Blocks serving | Fatal | Grows with | Recommendation |
| --- | --- | --- | --- | --- |
| Telemetry | yes | exporter build only | — | keep |
| Schema migrations | yes | yes | pending migrations | keep |
| Data migrations | yes | yes | pending migrations (none) | keep the runner |
| Connection pool | yes | yes | — | keep |
| Photo dimension backfill | no | no | photos (nothing pending) | **delete** |
| Backup scheduler | no | dir creation only | database size (snapshot if overdue) | keep |
| `reconcile_categories` | yes | yes | shopping items | keep |
| `resume_interrupted_jobs` | yes | no | interrupted jobs | keep |
| `sync_with_catalog` | yes | yes | resolved names; shopping items after a catalog change | keep |
| `queue_stored` | yes | yes | recipes × ingredients | keep for now |
| Name worker | no | no | pending names | keep |
| Router and listener | yes | yes | — | keep |

Step 10 could run as the name worker's first step, after the server is
listening. That would take about 270 ms off startup. It would also turn a
panic at startup into a logged error that the worker retries. Step 9 has to
stay ahead of serving (see above). Step 10 is worth moving if the collection
grows by an order of magnitude or startup becomes noticeable. At today's
size, keeping it simple and fatal costs less.
