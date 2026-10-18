# One sync design

Status: approved by the owner on 2026-10-17 and implemented (issue `one-sync-mechanism`).

## Before this change

Change stamping was already unified. A one-row `sync_clock` counter is bumped
by triggers inside each write's transaction (`recipe_versions.change_seq`,
`recipes.deleted_seq`, `user_tags.change_seq`, `shopping_list_items.change_seq`).
A cursor is "one past the clock" (`db::sync_cursor`). A cursor above the clock
(from another database) or at or below `resync_floor` (set when a backup is
restored) gets a full resync. The wall-clock `last_sync_at` path was gone.

What differed was everything layered on top of the clock:

| | Recipes | Shopping list (iOS) | Shopping list (web) | Tags | Meal plans |
|---|---|---|---|---|---|
| Read feed | `GET /api/recipes/sync`, keyset pages | `POST /api/shopping-list/sync` | same endpoint, reads only | none (names inside recipes, plus a `GET /api/tags` snapshot) | none; fetched per week |
| Write path | REST, online | sync endpoint, offline outbox | per-item `PUT`/`DELETE`/`POST`, online | REST, online | REST, online |
| Conflict check | `expected_version_id`, 409 | whole-row `expected_version`; a rejection takes the server copy | none (last write wins per field) | none | none |
| Full resync | all live rows + every tombstone; client prunes by page ID range | all live rows (+ every tombstone after a restore); client prunes | replaces the cache | n/a | n/a |
| Response naming | `recipes`, `deleted`, `has_more` | `server_changes`, `deleted`, `created`, `updated` | | | |

The real defect was the shopping-list conflict policy, not the plumbing:

* iOS sends the **whole row** with `expected_version`, so any concurrent change
  on another device rejects the update. The 2026-10-11 stopgap
  (`ShoppingListSyncSupport.reconcileSyncedItem`) then takes the server's copy,
  which also throws away an edit made on the phone while that sync was in
  flight. Example: the phone checks "eggs" while the web renames it to "6
  eggs". The phone's update is rejected and the check is lost.
* The web gets last-write-wins **per field** for the same table, because its
  `PUT /api/shopping-list/{id}` sends only the fields that changed and no
  version. The two clients resolve the same race in opposite ways.

Smaller inconsistencies:

* The iOS app resets the recipe cursor on `tagsDidChange`, which forces a full
  recipe resync after every tag rename, even though the server already resends
  the recipes affected by a tag change.
* Full resyncs ship every tombstone even though clients prune omitted rows
  anyway.
* Shopping items have both a `version` column and a `change_seq` stamp. They
  do overlapping jobs.

## Design

### 1. One protocol, one endpoint per domain

Every synced domain uses the same cursor rules, the same response envelope,
and the same server helpers (`server/src/api/sync/`). Domains keep separate
endpoints so the recipe corpus, which is large, can page while the small
domains answer in one response:

```
GET  /api/sync/recipes?cursor=&after_id=&limit=   read-only feed, keyset pages
GET  /api/sync/tags?cursor=                       read-only feed
POST /api/sync/shopping-list  { cursor, ops }     feed + writes
POST /api/sync/meal-plans     { cursor, ops }     feed + writes
```

Every response carries:

```
changes:     [Row]     full current rows changed at or after `cursor`
deleted:     [Uuid]    tombstones (on a full resync, only refused ops' rows)
cursor:      i64       pass back on the next sync
full_resync: bool      `changes` is every live row; drop cached rows it omits
```

The recipe feed adds `has_more` and `normalization_contract_version`. The
shopping list adds `category_order`. Write endpoints add `results` (section 3).

They replace `GET /api/recipes/sync`, `POST /api/shopping-list/sync`, the
per-item shopping-list REST endpoints (`GET`/`POST /api/shopping-list`,
`PUT`/`DELETE /api/shopping-list/{id}`, `DELETE /api/shopping-list/clear-checked`),
and the meal-plan write endpoints (`POST /api/meal-plans`, `PUT`/`DELETE
/api/meal-plans/{id}`). Web keeps reading meal plans one week at a time
from `GET /api/meal-plans`. Tag rename, delete and create stay REST, and so
does every recipe write.

### 2. Change tracking and cursors

* Every row in a synced table carries `change_seq`. A trigger stamps it from
  `sync_clock`, and application code never writes it. `meal_plans` gains the
  column and its triggers. A recipe's own row keeps `deleted_seq`: the
  recipe's content is versioned, so its delete is the only change on the
  row.
* Shopping items stop using `version`, and the per-row `change_seq` takes
  over its job. Rows on writable feeds carry `change_seq`. A client ignores a server
  row older than the copy it holds, except on a full resync, whose stamps may
  come from another database.
* The cursor format is unchanged: an opaque `i64` meaning "one past the
  clock". Restores keep the current behavior: `restamp_live` jumps the clock
  by 2^40, sets `resync_floor`, and restamps every live row in every synced
  table (now including meal plans). Every older cursor, and every foreign
  cursor (one above the clock), gets a full resync.
* **Full resync** sends every live row and no tombstones, and the client
  drops any cached row the response omits. Recipe pages keep their per-page
  ID-range pruning and their restart rule.
* **Writes are in the same response.** A write endpoint applies its ops
  first and then reads the cursor and the changes. The write lock is held
  throughout, so a client gets its own writes back as ordinary changes, and
  the next sync doesn't redeliver them.
* **A refused op's row is settled by the feed.** For every op that wasn't
  applied, the response carries the server's truth whatever the cursor says:
  the live row in `changes`, or else its id in `deleted` (even on a full
  resync). Clients then need no special handling of op results beyond
  telling the user, and the shared apply logic settles the cache.
* **Derived rows.** A recipe's tag names come from `user_tags`, so a tag
  change resends the recipes that carry it (as today). A meal plan's recipe
  title and thumbnail come from the recipe's current version, so a new
  version resends its meal plans, and deleting a recipe sends its meal plans
  as tombstones.

### 3. Writes: client-chosen ids, field ops, per-field last-writer-wins

```
ops: [
  { op: "create", id, fields: {...} },      // id generated by the client
  { op: "update", id, fields: {...} },      // only the fields the user changed
  { op: "delete", id },
]
results: [ "applied" | "gone" | "duplicate" | "recipe_missing" ]  // one per op, in order
```

* **Creates use the client's id as the row id.** Replaying a create is a
  no-op, and no `client_id` mapping step is needed. The old `client_id` and
  `version` columns stay in place, unused, because live data is never
  dropped. A create whose id already exists for another user is `gone`.
* **Updates name only changed fields.** `""` clears a nullable text field,
  and a field left out stays as it is. (`null` can't mean "clear": the
  generated Swift encoder drops nil fields. This replaces
  `clear_category_override`.)
  The server applies ops in arrival order. Two devices editing different
  fields both win. When they edit the same field, the later arrival wins.
  There is no `expected_version` and no rejection, so the iOS stopgap and its
  lost-edit bug go away.
* **Delete wins over edit.** An `update` of a deleted or unknown row returns
  `gone` and changes nothing, and the client drops the row. A `delete` of an
  already-deleted row is `applied`.
* Meal plans keep their "one recipe per slot" rule. A create or update that
  would put the same recipe in the same date and meal twice returns
  `duplicate`. A meal-plan create naming a missing or deleted recipe returns
  `recipe_missing`. Either way the client retakes the server's copy (or drops
  a create).
* Requests that are malformed outright, such as an invalid category, fail
  as a whole with 400.
* **iOS** keeps a durable outbox for each writable domain: the pending
  create or delete, or for an update its set of dirty fields. A server row
  overwrites only fields that aren't dirty. A successful sync clears a dirty
  field only if its value is still the one that was sent, so an edit made
  mid-sync stays dirty and goes out next round. A local delete always goes
  through the server, even for an item whose create looks unsent: the create
  may have reached the server on a sync whose response was lost.
* **Web** sends the same ops, online only (doc/web-sync.md still holds, so
  web has no outbox). "Clear checked" becomes a `delete` op for each checked
  item the page shows, so web never deletes an item checked on another
  device that the page hasn't seen yet.
* **Recipes stay document-level optimistic**: `PUT` with
  `expected_version_id`, 409 on mismatch, and the user chooses. Recipe bodies
  are ordered, structured documents, so a field merge of ingredients or
  instructions would do more harm than a prompt (see
  doc/local-recipe-search-and-sync.md, "Full local-first recipe editing").
* **Tags**: rename and delete stay online REST, last write wins. A rename
  colliding with an existing name stays 409.

### 4. Deletes

Deletes are always soft (`deleted_at`). The trigger stamps them, so each
appears as a tombstone in the next delta. Clients never need old tombstones:
a cursor too old to answer gets a full resync, and pruning covers it.

### 5. Feeds per domain

* **Recipes**: the existing `SyncRecipe` rows, renamed to `changes`.
* **Tags**: `{ id, name, change_seq }`. Clients parse namespaces from names, as they
  already did. On iOS this replaces the `listAllTags` snapshot behind the
  filter chips and the recipe form's suggestions, and the recipe cursor is no
  longer reset on `tagsDidChange`. Tag counts stay on `GET /api/tags` for the
  management screen.
* **Shopping list**: the current item row without `version` and
  `updated_at`, plus `change_seq`.
* **Meal plans**: `{ id, recipe_id, recipe_title, thumbnail_photo_id,
  meal_date, meal_type, notes, change_seq }`, covering every live plan whose
  recipe is live. iOS caches them in Core Data and shows the meal-plan week
  from the cache, with an outbox like the shopping list's. It syncs when the
  account is set, when the meal-plan tab appears, on pull-to-refresh, and
  after each edit. Before saving an add or a move, it checks the cache for
  the same recipe in the same meal. The server still has the final say, and
  a refusal shows once as an alert.

## Migration of installed builds

There's one user and backward compatibility isn't required (AGENTS.md), so the
old endpoints and fields are removed outright. The only data at risk is
**unsent iOS shopping-list edits** in the Core Data outbox:

* The Core Data model becomes versioned (`Ramekin 2.xcdatamodel`, beside the
  unchanged original), so lightweight migration has the source model it
  needs. Version 2 adds `changeSeq` and `dirtyFields` to `ShoppingItem`,
  drops its `serverVersion` and `clearCategoryOverride`, and adds the
  `CachedMealPlan` entity.
* An existing `pending_update` row with no dirty fields recorded counts as
  "all fields dirty", which replays the old whole-row update once, without a
  version check.
* `pending_delete` rows map directly to `delete` ops. A `pending_create` row
  sends its local id, which the old protocol sent as `client_id`, as the new
  row's id. One edge case remains: a create whose old-protocol response
  never arrived exists on the server under a server-chosen id, and it gets
  duplicated. Delete the duplicate by hand.
* The shopping-list cursor moves to a new storage key, and meal plans and
  tags start with no cursor, so the first sync of each is a full resync. The
  recipe cursor carries over, since its format didn't change.

## Testing

* e2e (`tests/`): the shared envelope on every feed. Op semantics: different
  fields both win, the later write to one field wins, an `update` after a
  delete returns `gone`, creates are idempotent, and meal plans return
  `duplicate` and `recipe_missing`. A full resync carries no tombstones, and
  a restored or foreign cursor gets one. Keep the race tests
  (`test_recipe_sync_races.py`).
* Applying a response to a cache runs on both clients: tombstones,
  full-resync pruning, skipping rows older than the cached copy, and keeping
  dirty fields. Pin it with `shared-test-vectors/sync-apply.json`, consumed
  by Vitest and XCTest, per doc/client-logic-sharing.md.

## Implementation

Server, web and iOS ship in one PR. The API break lands all at once, and the
owner updates every client together.
