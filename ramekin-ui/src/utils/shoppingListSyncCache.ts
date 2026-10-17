import type {
  OpResult,
  ShoppingListOp,
  SyncApi,
  SyncShoppingListItem,
  SyncShoppingListResponse,
} from "ramekin-client";
import { applySyncResponse } from "./syncApply";

// Version 3 moved to the one-protocol sync (doc/sync-design.md): items carry
// `changeSeq` instead of `version` and `updatedAt`.
const CACHE_VERSION = 3;
const CACHE_KEY_PREFIX = "shoppingListSyncCache";

export interface ShoppingListSyncCache {
  version: typeof CACHE_VERSION;
  items: SyncShoppingListItem[];
  categoryOrder: string[];
  cursor: number | null;
}

export interface ShoppingListSyncResult {
  cache: ShoppingListSyncCache;
  results: OpResult[];
}

export function shoppingListCacheKey(token: string): string {
  return `${CACHE_KEY_PREFIX}:${fingerprint(token)}`;
}

export function loadShoppingListSyncCache(
  storage: Pick<Storage, "getItem">,
  token: string | null,
): ShoppingListSyncCache | null {
  if (!token) return null;

  const raw = storage.getItem(shoppingListCacheKey(token));
  if (raw === null) return null;

  const parsed = JSON.parse(raw) as unknown;
  return parseCache(parsed);
}

export function saveShoppingListSyncCache(
  storage: Pick<Storage, "setItem">,
  token: string | null,
  cache: ShoppingListSyncCache,
): void {
  if (!token) return;

  storage.setItem(shoppingListCacheKey(token), JSON.stringify(cache));
}

export function clearShoppingListSyncCache(
  storage: Pick<Storage, "removeItem">,
  token: string | null,
): void {
  if (!token) return;

  storage.removeItem(shoppingListCacheKey(token));
}

/**
 * The cache after a sync: the response applied, then every item an op found
 * gone dropped (it may have been deleted too long ago for a tombstone).
 */
export function applyShoppingListSyncResponse(
  cache: ShoppingListSyncCache | null,
  response: SyncShoppingListResponse,
  ops: readonly ShoppingListOp[] = [],
): ShoppingListSyncCache {
  const gone = new Set(
    ops.filter((_, i) => response.results[i] === "gone").map((op) => op.id),
  );
  return {
    version: CACHE_VERSION,
    items: applySyncResponse(cache?.items ?? [], response).filter(
      (item) => !gone.has(item.id),
    ),
    categoryOrder: response.categoryOrder,
    cursor: response.cursor,
  };
}

// One sync at a time: a response computed before another sync's writes
// would otherwise land after them and bring back what they deleted.
let inFlight: Promise<unknown> = Promise.resolve();

/**
 * Send `ops` (none for a plain refresh) from the cached cursor, and store
 * the result. A cache that can't be read is synced from scratch; one that
 * can't be written is cleared, via `onCacheError`.
 */
export function syncShoppingList(
  api: Pick<SyncApi, "syncShoppingList">,
  storage: Pick<Storage, "getItem" | "setItem" | "removeItem">,
  token: string | null,
  ops: ShoppingListOp[] = [],
  onCacheError: (context: string, err: unknown) => void = () => {},
): Promise<ShoppingListSyncResult> {
  const run = async () => {
    let cached: ShoppingListSyncCache | null = null;
    try {
      cached = loadShoppingListSyncCache(storage, token);
    } catch (err) {
      onCacheError("shopping list cache read failed", err);
      clearQuietly(storage, token);
    }
    const response = await api.syncShoppingList({
      syncShoppingListRequest: { cursor: cached?.cursor ?? undefined, ops },
    });
    const cache = applyShoppingListSyncResponse(cached, response, ops);
    try {
      saveShoppingListSyncCache(storage, token, cache);
    } catch (err) {
      onCacheError("shopping list cache write failed", err);
      clearQuietly(storage, token);
    }
    return { cache, results: response.results };
  };
  const result = inFlight.then(run, run);
  inFlight = result.catch(() => {});
  return result;
}

function clearQuietly(
  storage: Pick<Storage, "removeItem">,
  token: string | null,
): void {
  try {
    clearShoppingListSyncCache(storage, token);
  } catch {
    // Nothing more to do: the next read fails and syncs from scratch.
  }
}

function parseCache(value: unknown): ShoppingListSyncCache {
  if (!isRecord(value)) {
    throw new Error("Shopping list sync cache is not an object");
  }
  if (value.version !== CACHE_VERSION) {
    throw new Error("Shopping list sync cache has an unsupported version");
  }
  if (!Array.isArray(value.items)) {
    throw new Error("Shopping list sync cache items are invalid");
  }
  if (!Array.isArray(value.categoryOrder)) {
    throw new Error("Shopping list sync cache category order is invalid");
  }

  const cursor =
    value.cursor === null ? null : requiredNumber(value.cursor, "cursor");
  const items = value.items.map(parseCachedItem);
  const categoryOrder = value.categoryOrder.map((category) => {
    if (typeof category !== "string") {
      throw new Error("Shopping list sync cache category order is invalid");
    }
    return category;
  });

  return {
    version: CACHE_VERSION,
    items,
    categoryOrder,
    cursor,
  };
}

function parseCachedItem(value: unknown): SyncShoppingListItem {
  if (!isRecord(value)) {
    throw new Error("Shopping list sync cache item is not an object");
  }

  const id = requiredString(value.id, "id");
  const item = requiredString(value.item, "item");
  const category = requiredString(value.category, "category");
  const computedCategory = requiredString(
    value.computedCategory,
    "computedCategory",
  );
  const isChecked = requiredBoolean(value.isChecked, "isChecked");
  const sortOrder = requiredNumber(value.sortOrder, "sortOrder");
  const changeSeq = requiredNumber(value.changeSeq, "changeSeq");

  return {
    amount: optionalString(value.amount, "amount"),
    category,
    categoryOverride: optionalString(
      value.categoryOverride,
      "categoryOverride",
    ),
    changeSeq,
    computedCategory,
    id,
    isChecked,
    item,
    note: optionalString(value.note, "note"),
    sortOrder,
    sourceRecipeId: optionalString(value.sourceRecipeId, "sourceRecipeId"),
    sourceRecipeTitle: optionalString(
      value.sourceRecipeTitle,
      "sourceRecipeTitle",
    ),
  };
}

function requiredString(value: unknown, field: string): string {
  if (typeof value !== "string") {
    throw new Error(`Shopping list sync cache ${field} is not a string`);
  }
  return value;
}

function optionalString(value: unknown, field: string): string | undefined {
  if (value === undefined || value === null) return undefined;
  if (typeof value !== "string") {
    throw new Error(`Shopping list sync cache ${field} is not a string`);
  }
  return value;
}

function requiredBoolean(value: unknown, field: string): boolean {
  if (typeof value !== "boolean") {
    throw new Error(`Shopping list sync cache ${field} is not a boolean`);
  }
  return value;
}

function requiredNumber(value: unknown, field: string): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw new Error(`Shopping list sync cache ${field} is not a number`);
  }
  return value;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function fingerprint(value: string): string {
  let hash = 5381;
  for (let i = 0; i < value.length; i += 1) {
    hash = (hash * 33) ^ value.charCodeAt(i);
  }
  return (hash >>> 0).toString(36);
}
