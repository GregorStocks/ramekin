import type {
  ShoppingListItemResponse,
  SyncResponse,
  SyncServerChange,
} from "ramekin-client";

const CACHE_VERSION = 1;
const CACHE_KEY_PREFIX = "shoppingListSyncCache";

export interface ShoppingListSyncCache {
  version: typeof CACHE_VERSION;
  items: ShoppingListItemResponse[];
  categoryOrder: string[];
  lastSyncAt: Date | null;
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

  storage.setItem(
    shoppingListCacheKey(token),
    JSON.stringify({
      ...cache,
      lastSyncAt: cache.lastSyncAt?.toISOString() ?? null,
    }),
  );
}

export function applyShoppingListSyncResponse(
  cache: ShoppingListSyncCache | null,
  response: SyncResponse,
): ShoppingListSyncCache {
  const deleted = new Set(response.deleted);
  const byId = new Map<string, ShoppingListItemResponse>();

  for (const item of cache?.items ?? []) {
    if (!deleted.has(item.id)) {
      byId.set(item.id, item);
    }
  }

  for (const change of response.serverChanges) {
    if (!deleted.has(change.id)) {
      byId.set(change.id, itemFromServerChange(change));
    }
  }

  return {
    version: CACHE_VERSION,
    items: Array.from(byId.values()),
    categoryOrder: response.categoryOrder,
    lastSyncAt: response.syncTimestamp,
  };
}

function itemFromServerChange(
  change: SyncServerChange,
): ShoppingListItemResponse {
  return {
    amount: change.amount,
    category: change.category,
    categoryOverride: change.categoryOverride,
    computedCategory: change.computedCategory,
    id: change.id,
    isChecked: change.isChecked,
    item: change.item,
    note: change.note,
    sortOrder: change.sortOrder,
    sourceRecipeId: change.sourceRecipeId,
    sourceRecipeTitle: change.sourceRecipeTitle,
    updatedAt: change.updatedAt,
    version: change.version,
  };
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

  const lastSyncAt = parseNullableDate(value.lastSyncAt);
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
    lastSyncAt,
  };
}

function parseCachedItem(value: unknown): ShoppingListItemResponse {
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
  const version = requiredNumber(value.version, "version");
  const updatedAt = requiredDate(value.updatedAt, "updatedAt");

  return {
    amount: optionalString(value.amount, "amount"),
    category,
    categoryOverride: optionalString(
      value.categoryOverride,
      "categoryOverride",
    ),
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
    updatedAt,
    version,
  };
}

function parseNullableDate(value: unknown): Date | null {
  if (value === null) return null;
  return requiredDate(value, "lastSyncAt");
}

function requiredDate(value: unknown, field: string): Date {
  if (typeof value !== "string") {
    throw new Error(`Shopping list sync cache ${field} is not a string`);
  }
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    throw new Error(`Shopping list sync cache ${field} is not a valid date`);
  }
  return date;
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
