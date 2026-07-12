# Local recipe search and sync strategy

Decision record for issue `p4-sqlite-local-first-exploration` (2026-07-11).

## Decision

Keep PostgreSQL as the server database. PR #643 extended the existing read-only
recipe sync and Core Data cache with the structured recipe body needed by local
search. Finish offline iOS search on that path by making result membership
exact and mirroring the server's deterministic matching and scoring logic in
Swift through shared normalization, matching, filtering, and ranking contracts.

Do not build offline recipe writes or adopt a generic replication system until
there is a concrete product requirement for editing recipes offline. If that
requirement arrives, first extend the existing application-specific sync with
client-generated IDs, an outbox, and optimistic version checks. Reconsider a
replication product only if multiple substantial domains need offline writes.

This deliberately separates three decisions that do not need to move together:

1. iOS needs a complete local search corpus.
2. Recipe mutations might someday need to work offline.
3. The server could use SQLite instead of PostgreSQL.

Only the first is needed for local search. Changing the server database would
not put server data on a phone; a synchronization protocol would still be
required.

## What exists today

### Recipe reads on iOS

The iOS app already has a SQLite-backed Core Data store. Since PR #643,
`RecipeCacheStore` stores every active recipe's summary, structured ingredients,
instructions, and notes for an account in `CachedRecipe`. The endpoint
`GET /api/recipes/sync` maintains it using an opaque integer cursor:

- The first request returns all active recipe summaries.
- Later requests return current recipe versions changed at or after the previous
  cursor, plus recipes whose tags changed.
- Soft-deleted recipe IDs are returned as tombstones.

The cursor is a PostgreSQL transaction-id watermark, not a wall clock. Every
sync-visible change carries the 64-bit id of the transaction that wrote it
(`recipe_versions.change_xid`, `recipes.deleted_xid`, `user_tags.change_xid`,
all stamped by `current_change_xid()`). The sync reads in a single read-only
repeatable-read transaction and returns `change_xid_watermark()` — the snapshot's
`pg_snapshot_xmin`, the lowest transaction id still in flight — as the next
cursor. Every transaction that has not committed-and-become-visible by that
snapshot necessarily has an id at or above the watermark, so the next request's
inclusive `>= cursor` filter returns it.

That ordering is what a wall-clock cursor could not provide: a transaction took
its timestamp before the cursor, committed after the sync SELECT's snapshot, and
was then excluded by the next request's strict `>` filter, permanently omitting a
recipe update, tag change, or tombstone. The trade is deliberate — the watermark
lags any in-flight writer, so changes can be *redelivered* across syncs. Clients
already apply changes idempotently (upsert by id, tombstone by id), and
redelivering a change is recoverable where skipping one is not.
`tests/test_recipe_sync_races.py` pins this by stalling a real API write
mid-transaction, syncing, and then letting the write commit.

PR #643 also versioned the cache timestamp key, forcing existing summary-only
installs through one full refresh, and added API and iOS tests for population,
updates, deletions, and schema-version invalidation.

The cache serves offline browsing, tag filters, the basic photo-presence
filter, date filters, deterministic browse sorts, and — since the
`ios-local-search-relevance` work — bare-text search with server-identical
membership, relevance ranking, and tie-breaks. Queries involving source,
photo size or dimensions, or random sorting still go to the paginated server
endpoint. `doc/web-sync.md` separately records why the web cookbook remains
server-backed and paginated.

Core Data already uses an SQLite persistent store in the app-group container.
"Move the iOS cache to SQLite" is therefore not a meaningful first step. Core
Data's SQLite schema is private and must not be opened or modified through the
native SQLite API, but it is a suitable object store for the current cache.

### Search data and semantics

Server search first matches bare text against five field families, then ranks
the matches with six field families through the pure function in
`ramekin-core/src/search.rs`. Tags can increase the score of a recipe that
matched another field, but bare text does not match tags; tags enter the result
set through explicit `tag:` filters.

`tag:` has its own comparison contract. The server compares the complete tag
value through PostgreSQL `CITEXT`: case-insensitive but accent-sensitive
equality, not unaccented substring matching. Thus `tag:dinner` matches `Dinner`,
while `tag:creme` does not match `Crème`. The Swift implementation needs a
dedicated comparator pinned to server-backed case and accent vectors; it must
not pass tag values through the bare-text normalizer.

| Field family | Cached after #643 | Bare-text match | Per-token ranking weight |
|---|---:|---:|---:|
| Title | yes | yes | 2,000, plus whole-title bonuses |
| Tags | yes | no (`tag:` only) | 800 |
| Description | yes | yes | 400 |
| Ingredients | structured values only | yes | 200 |
| Instructions | yes | yes | 50 |
| Notes | yes | yes | 50 |

Ingredients currently have two distinct search representations. The SQL match
casts the stored JSONB to text, so JSON keys and serialization syntax can make
a recipe match. The scorer instead flattens measurement values, item, note,
and section into human-facing text. A token found only in a JSON key can
therefore select a row and still contribute no ingredient score.

The cache now contains all six scorer field families and the human-facing data
for all five bare-text matching families.

`SyncRecipe` carries a server-produced `ingredient_match_text` equal to the
exact `ingredients::text` value used by the current SQL filter, persisted
beside the structured ingredients. If server search later changes to match only
flattened ingredient values, change the API, iOS, and shared vectors together.
Source and detailed photo metadata remain outside the search document. Text
queries combined with `source:`, `photo_size:`, or `photo_dim:` therefore stay
on the server, as does random browsing. Local search covers bare text, `tag:`,
basic photo presence, and created-date filters, with relevance, updated-date,
rating, created-date, and title browse ordering. (A text query's browse sort
is irrelevant on both sides — the app always asks for relevance — so only
random browsing without text terms needs the server.)

PR #643 made the initial sync larger; later syncs now transfer only changed
recipes. Immutable `recipe_versions` provide a stable version identity, and
`change_xid` orders those versions against commits in a way wall-clock
`created_at` cannot. Before enabling local search, measure stored cache size and search
latency against representative large cookbooks. That measurement should choose
the query implementation; it should not hold the data model hostage to an
unmeasured indexing concern.

### Offline writes that already exist

The shopping list demonstrates the repository's application-specific
offline-write pattern:

- Core Data records pending creates, updates, and deletes.
- Creates carry a client-generated ID and are idempotent on the server.
- Updates carry an expected integer version and fail on a concurrent change.
- Deletes are server-side soft deletes.
- The sync response maps created IDs, reports update success and current
  versions, returns tombstones, and advances the timestamp.

This is already most of the machinery a single-user, multi-device recipe
workflow would need. It is intentionally explicit about domain behavior rather
than pretending transport removes the need to decide what a conflict means.

## Narrow path: local search without local-first writes

The smallest complete design is:

1. Done: the transaction-id watermark cursor above replaced the timestamp, and
   `tests/test_recipe_sync_races.py` covers update, tag change, and soft-delete
   tombstones with coordinated API regression tests.
2. Extend the existing `SyncRecipe` document with the server-produced
   `ingredient_match_text`. Do not assume encoding its already-synced structured
   ingredients on iOS recreates PostgreSQL's JSONB text.
3. Persist that match text beside the existing full-body Core Data cache.
4. Implement the server's matching, normalization, and relevance scoring in
   Swift, preserving the distinction between bare-text matching fields and
   explicit `tag:` filters. Following `doc/client-logic-sharing.md`, expand the
   existing ranking fixtures with shared end-to-end match/filter vectors: raw
   query plus recipe documents in, matched and ordered IDs out. Consume both
   suites from server tests and XCTest. The match/filter suite must cover AND
   semantics across fields, quoted terms, basic photo presence, created-date
   filters, relevance/updated-date/rating/created-date ordering, and
   accent/ligature normalization. For `tag:`, include whole-value vectors
   proving case-insensitive and accent-sensitive `CITEXT` parity. The suite must
   also cover a token found only in an ingredient JSON key, which matches today
   but may score zero. Scorer-only vectors cannot prove result-set parity.
5. Route a text query locally only when source, photo-size, photo-dimension, and
   title/random-sort requirements are absent. Preserve the existing server path
   for every unsupported combination instead of evaluating it against missing
   metadata or a different collation.
6. Search the cached corpus locally and preserve the server's score, recency,
   and ID tie-break order.
7. Keep web search on `GET /api/recipes`; web has neither an offline product
   requirement nor a local corpus, and replacing its paginated flow with an
   all-recipes sync would be a regression.

For a personal cookbook, a Swift scan over normalized strings is the first
implementation to benchmark. The server already loads one user's matching
rows and applies the scorer in memory, and the scorer is deliberately small.
The iOS implementation can precompute normalized search fields when applying
sync changes if repeated normalization is the bottleneck.

### Exact normalization is a prerequisite

A best-effort mirror of PostgreSQL `unaccent` would be safe for ranking
(PostgreSQL already selected each result) but not for local matching: a
character handled by the database but absent from the Rust and Swift mapping
could make iOS omit a server result.

This normalization contract applies only to bare-text matching and scoring.
Structured tag equality retains the separate accent-sensitive `CITEXT`
contract above.

Local matching therefore consumes an exact versioned contract:
`shared-test-vectors/search-normalization.json` pins the complete
per-codepoint `unaccent` dictionary and `lower()` mapping generated from the
live server (`scripts/generate-search-normalization.sh`), both the Rust and
Swift normalizers apply it verbatim, and
`tests/test_search_normalization_contract.py` verifies every mapping — in
both directions, over every Unicode codepoint — against the running
database's `f_unaccent` and `lower`. The sync response carries the contract
version and the app fails the sync when it does not support it, so the app
and server cannot silently advance to different mappings.

### When a direct SQLite layer would help

If measurements show that scanning misses the interactive latency budget on
supported devices, use a separate application-owned SQLite database through
GRDB rather than reaching through Core Data's private store. GRDB provides
migrations, observation, a Swift query interface, and FTS5 support.

FTS5 is not a drop-in replacement for Ramekin search:

- Ramekin uses accent-insensitive substring containment for every bare-text
  query token across its five matching fields.
- FTS5 normally tokenizes words; its trigram tokenizer can accelerate more
  general substring queries, but strings shorter than three characters need
  special handling.
- The canonical weighted scorer and deterministic tie-breaks must still run on
  the candidate rows.
- SQLite's built-in Unicode behavior does not exactly equal PostgreSQL
  `unaccent`, so final membership must use the exact versioned normalization
  contract described above.

An FTS table should therefore be an optimization that produces candidates,
not a second definition of matching or ranking. Adding GRDB also means another
dependency, schema, migration path, and cache invalidation surface. Those costs
are justified only by a measured problem.

## Full local-first recipe editing

Offline reads have no conflicts. Offline writes do, even when every account
belongs to one person: the same user can edit a recipe on a phone and a tablet
before either device receives the other's change.

The repository's immutable recipe versions make a narrow protocol feasible:

- Assign client-generated UUIDs to offline creates and make create replay
  idempotent.
- Queue mutations in a durable client outbox.
- Send an `expected_version_id` with an edit. The server creates a new
  immutable version only when the expected version is still current.
- On mismatch, retain both bodies and report a conflict. Do not silently
  overwrite a recipe or attempt a magic merge of instructions and ingredients.
- Keep deletions as tombstones and define delete-versus-edit behavior
  explicitly. A conservative first policy is to reject the stale edit and let
  the user restore or copy it.
- Treat photo uploads as a separate queued operation with retry and
  idempotency; large blobs should not complicate the recipe metadata cursor.
- Use the race-safe server cursor established for recipe reads for downloads,
  never the current wall-clock timestamp cursor or a device clock.

For this product, optimistic document-level conflict detection is preferable
to CRDTs. Concurrent edits should be rare, recipe bodies contain ordered and
structured content, and an automatic element-level merge can be more damaging
than asking which version to keep. Simple fields can acquire explicit merge
rules later if real conflict data warrants them. CRDTs become attractive only
for live collaborative editing or frequent independent field edits, neither of
which is a current requirement.

## Replication systems considered

The ecosystem was reviewed on 2026-07-11. Product capabilities and licensing
should be rechecked before any adoption.

### PowerSync

PowerSync is the closest match to the current stack. It has a native Swift SDK
that exposes a local SQLite database and streams changes from PostgreSQL. It can
be hosted or managed, and its Swift integration supports GRDB.

It does not eliminate application sync design. PostgreSQL must enable logical
replication and publish the selected tables. Client writes enter an upload
queue, but Ramekin must still implement the authenticated backend upload API,
validation, idempotency, and conflict rules. PowerSync documents a default
field-level last-write-wins policy and several custom version/conflict
strategies; choosing among them remains application work.

For a read-only recipe corpus, PowerSync would replace one small existing delta
endpoint with a service, replication publication, client SDK, operational
monitoring, and a second local storage API. That is a poor trade. It becomes
worth reevaluating if recipes, meal plans, shopping lists, tags, and other
domains all become independently writable offline and maintaining their sync
feeds dominates development.

### Ditto and Couchbase Lite

Ditto provides an embedded database and current Swift sync SDK with field-level
delta synchronization and CRDT-style concurrent-field merging. Couchbase Lite
provides a Swift embedded document database, continuous replication through
Sync Gateway, and version-vector/conflict-resolution machinery.

Both solve a broader distributed-data problem than local recipe search. They
would introduce a new data model and operational or hosted sync layer, while
Ramekin would still need to integrate PostgreSQL or replace it as the source of
truth. Their richer peer-to-peer and conflict capabilities have no identified
product use today. A backend migration to obtain those capabilities would cost
far more than extending the existing recipe sync.

### Core Data with CloudKit

Core Data can mirror a store through CloudKit, but that would make a user's
iCloud account part of the synchronization architecture. It does not naturally
preserve Ramekin's self-hosted server, bearer-authenticated accounts, web
client, or PostgreSQL source of truth. It is not a fit for this product.

### A custom CRDT layer

A custom CRDT or operation-log implementation has the highest maintenance risk.
It requires stable operation identities, causal metadata, compaction,
tombstones, schema evolution, deterministic merge rules for every field, and
extensive convergence testing. The single-user use case lowers conflict
frequency; it does not make those correctness requirements disappear. Do not
build this without a collaborative-editing requirement that optimistic
versioning demonstrably cannot serve.

## Why the server should stay on PostgreSQL

Diesel supports both PostgreSQL and SQLite, but this codebase is not backend
agnostic. The current schema and queries depend on PostgreSQL features:

- `UUID`, `TIMESTAMPTZ`, `BYTEA`, UUID arrays, `JSONB`, and `CITEXT` types.
- `pg_trgm` GIN indexes for substring search.
- `unaccent` plus an immutable wrapper used by expression indexes.
- Partial indexes, `gen_random_uuid()`, `array_agg`, `ANY`, `LATERAL`, array
  operations, JSONB-to-text casts, and PostgreSQL conflict clauses.
- `PgConnection` in the pool and query helpers.
- PostgreSQL-specific raw SQL fragments for tags, photo filtering, and search.

SQLite now has capable JSON functions and its own binary JSONB representation,
but SQLite JSONB is not binary-compatible with PostgreSQL JSONB and has
different lookup and indexing characteristics. FTS5 is capable, but it would
replace rather than preserve the current trigram/unaccent behavior. Every
migration and affected query would need to be redesigned and retested.

Even after that work, each iPhone would still have a different SQLite file from
the server. The difficult part of local-first architecture is synchronizing
those files and resolving concurrent writes, not selecting the same storage
engine on both ends. A server database migration provides no shortcut.

## Staged recommendation

### Stage 1: complete local reads

PR #643 completed the full-body read-only cache, and the transaction-id
watermark cursor made its deltas race-safe. Next add the distinct ingredient
match text and versioned normalization contract to `SyncRecipe`, and persist
them in the existing cache.

This stage is complete: sync carries `ingredient_match_text` and a versioned
normalization contract (`shared-test-vectors/search-normalization.json`,
verified against the live database by
`tests/test_search_normalization_contract.py`), Swift mirrors query parsing,
matching, filtering, scoring, and ordering
(`ramekin-ios/Ramekin/RecipeSearchSupport.swift`), and the behavior is pinned
end to end by `shared-test-vectors/search-match-filter.json` and
`search-ranking.json`, consumed by both the Python API tests and XCTest. Text
queries now serve from the cache unless they need `source:`, `photo_size:`,
or `photo_dim:`; random browsing without text terms stays on the server. Web
remains unchanged.

### Stage 2: optimize only from measurements

If normalization and scanning are slow, first cache normalized strings. If a
scan is still too slow, prototype a GRDB/FTS5 trigram candidate index and prove
with the shared vectors that the final results remain identical. Keep Core Data
unless maintaining two stores is measurably better than moving the cache.

### Stage 3: add offline mutations only for a real workflow

If offline recipe editing becomes a product requirement, extend the existing
sync conventions with a recipe outbox, client IDs, expected version IDs,
tombstones, and an explicit conflict response. Preserve both immutable versions
when edits conflict and collect evidence before designing finer merge rules.

### Stage 4: revisit replication when custom sync becomes the product

Evaluate PowerSync or another replication layer when at least several domains
need robust offline writes, real-time cross-device updates, and independently
maintained cursors/outboxes. At that point the operational dependency may be
cheaper than continuing to grow application-specific sync. Do not migrate the
server to SQLite as part of that decision; PostgreSQL is directly supported by
the strongest candidate and remains the appropriate source of truth.

## Sources

- [Apple: Core Data persistent store types and behaviors](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/CoreData/PersistentStoreFeatures.html)
- [Apple: Core Data overview](https://developer.apple.com/documentation/coredata)
- [GRDB: SQLite toolkit for Swift](https://github.com/groue/GRDB.swift)
- [SQLite: FTS5 extension](https://www.sqlite.org/fts5.html)
- [SQLite: JSON functions and SQLite JSONB](https://www.sqlite.org/json1.html)
- [PowerSync: Swift SDK](https://docs.powersync.com/client-sdks/reference/swift)
- [PowerSync: source database setup](https://docs.powersync.com/configuration/source-db/setup)
- [PowerSync: writing client changes](https://docs.powersync.com/handling-writes/writing-client-changes)
- [PowerSync: custom conflict resolution](https://docs.powersync.com/handling-writes/custom-conflict-resolution)
- [Ditto: Swift SDK release notes](https://docs.ditto.live/sdk/latest/release-notes/swift)
- [Couchbase Lite: Swift replication](https://docs.couchbase.com/couchbase-lite/current/swift/replication.html)
- [Couchbase Lite 4.0 version vectors](https://docs.couchbase.com/couchbase-lite/current/cbl-whatsnew.html)
