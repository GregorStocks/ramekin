# Local recipe search and sync strategy

Decision record for issue `p4-sqlite-local-first-exploration` (2026-07-11).

## Decision

Keep PostgreSQL as the server database. For offline iOS recipe search, extend
the existing read-only recipe sync and Core Data cache to include the full
search document, then mirror the server's deterministic matching and scoring
logic in Swift using the shared vectors already planned for that work.

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

The iOS app already has a SQLite-backed Core Data store. `RecipeCacheStore`
stores every active recipe summary for an account in `CachedRecipe`, and
`GET /api/recipes/sync` maintains it using a server timestamp:

- The first request returns all active recipe summaries.
- Later requests return current recipe versions created after the previous
  timestamp, plus recipes whose tags changed.
- Soft-deleted recipe IDs are returned as tombstones.
- The server captures the next timestamp before querying and bounds results by
  that timestamp, so writes during a sync are not skipped.

The cache serves offline browsing, tag filters, the basic photo-presence
filter, date filters, and deterministic browse sorts. It intentionally sends
queries involving text, source, photo size or dimensions, or random sorting to
the paginated server endpoint. `doc/web-sync.md` separately records why the web
cookbook remains server-backed and paginated.

Core Data already uses an SQLite persistent store in the app-group container.
"Move the iOS cache to SQLite" is therefore not a meaningful first step. Core
Data's SQLite schema is private and must not be opened or modified through the
native SQLite API, but it is a suitable object store for the current cache.

### Search data and semantics

Server search matches six field families and then ranks the matches with the
pure function in `ramekin-core/src/search.rs`:

| Field family | In the iOS cache | Per-token ranking weight |
|---|---:|---:|
| Title | yes | 2,000, plus whole-title bonuses |
| Tags | yes | 800 |
| Description | yes | 400 |
| Ingredients | no | 200 |
| Instructions | no | 50 |
| Notes | no | 50 |

The summary cache therefore has 3 of 6 searchable field families and 3,200 of
the 3,500 ordinary per-token ranking points. It also has the title fields used
by the 10,000 to 100,000 point whole-title bonuses. That is enough data to rank
many common searches plausibly, but it is not enough to search correctly: a
token present only in an ingredient, instruction, or note must still make a
recipe a match. Summary-only local search would produce false negatives.

Adding ingredients, instructions, and notes to the synced search document
closes that entire correctness gap. It supports all six matching fields and
100% of the scorer's inputs without enabling a single offline mutation.
Source and photo metadata can remain outside the search document unless local
versions of those filters are also requested.

The full-body cache will make the initial sync larger, but later syncs still
transfer only changed recipes. Recipe updates already create immutable
`recipe_versions`, so the current version's `created_at` is an effective change
marker. Before implementation, measure initial payload size, stored cache size,
sync duration, and search latency against representative large cookbooks. That
measurement should choose the query implementation; it should not hold the
data model hostage to an unmeasured indexing concern.

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

1. Make recipe sync return a search document containing title, description,
   tags, ingredients, instructions, and notes. This can be a purpose-specific
   sync model rather than every field in `RecipeResponse`.
2. Persist that document in the existing account-scoped Core Data cache.
3. Implement the server's matching, normalization, and relevance scoring in
   Swift. Consume the same JSON ranking vectors as the Rust tests, following
   `doc/client-logic-sharing.md`.
4. Search the cached corpus locally and preserve the server's score, recency,
   and ID tie-break order.
5. Keep web search on `GET /api/recipes`; web has neither an offline product
   requirement nor a local corpus, and replacing its paginated flow with an
   all-recipes sync would be a regression.

For a personal cookbook, a Swift scan over normalized strings is the first
implementation to benchmark. The server already loads one user's matching
rows and applies the scorer in memory, and the scorer is deliberately small.
The iOS implementation can precompute normalized search fields when applying
sync changes if repeated normalization is the bottleneck.

### When a direct SQLite layer would help

If measurements show that scanning misses the interactive latency budget on
supported devices, use a separate application-owned SQLite database through
GRDB rather than reaching through Core Data's private store. GRDB provides
migrations, observation, a Swift query interface, and FTS5 support.

FTS5 is not a drop-in replacement for Ramekin search:

- Ramekin uses accent-insensitive substring containment for every query token.
- FTS5 normally tokenizes words; its trigram tokenizer can accelerate more
  general substring queries, but strings shorter than three characters need
  special handling.
- The canonical weighted scorer and deterministic tie-breaks must still run on
  the candidate rows.
- SQLite's built-in Unicode behavior does not exactly equal PostgreSQL
  `unaccent`, so the shared normalizer remains the source of truth.

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
- Use a server-issued cursor or the existing bounded server timestamp for
  downloads, never a device clock.

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

Implement `p3-ios-full-recipe-cache-local-search`: sync the six-field search
document into Core Data, measure payload/storage/search performance, and cover
population, updates, tag changes, and tombstones with API and iOS tests.

Then implement `blocked-ios-local-search-relevance`: mirror matching and scoring
in Swift and pin it to the shared Rust vectors. This delivers instant offline
search without any conflict or upload path. Web remains unchanged.

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
