import { describe, expect, it } from "vitest";
import {
  applyShoppingListSyncResponse,
  clearShoppingListSyncCache,
  loadShoppingListSyncCache,
  saveShoppingListSyncCache,
  shoppingListCacheKey,
  syncShoppingList,
  type ShoppingListSyncCache,
} from "./shoppingListSyncCache";
import type {
  ShoppingListOp,
  SyncShoppingListItem,
  SyncShoppingListResponse,
} from "ramekin-client";

class MemoryStorage {
  private values = new Map<string, string>();

  getItem(key: string): string | null {
    return this.values.get(key) ?? null;
  }

  setItem(key: string, value: string): void {
    this.values.set(key, value);
  }

  removeItem(key: string): void {
    this.values.delete(key);
  }
}

describe("shopping list sync cache", () => {
  it("round-trips cached items and cursor through storage", () => {
    const storage = new MemoryStorage();
    const cache: ShoppingListSyncCache = {
      version: 3,
      categoryOrder: ["Produce", "Other"],
      items: [
        shoppingItem({
          id: "item-1",
          item: "apples",
          category: "Produce",
          changeSeq: 7,
        }),
      ],
      cursor: 41,
    };

    saveShoppingListSyncCache(storage, "token-a", cache);

    expect(loadShoppingListSyncCache(storage, "token-a")).toEqual(cache);
  });

  it("keeps cache entries separated by token", () => {
    const storage = new MemoryStorage();
    saveShoppingListSyncCache(storage, "token-a", emptyCache());

    expect(loadShoppingListSyncCache(storage, "token-b")).toBeNull();
  });

  it("clears cached data for one token", () => {
    const storage = new MemoryStorage();
    saveShoppingListSyncCache(storage, "token-a", emptyCache());
    saveShoppingListSyncCache(storage, "token-b", emptyCache());

    clearShoppingListSyncCache(storage, "token-a");

    expect(loadShoppingListSyncCache(storage, "token-a")).toBeNull();
    expect(loadShoppingListSyncCache(storage, "token-b")).toEqual(emptyCache());
  });

  it("applies changes and tombstones to cached items", () => {
    const cached: ShoppingListSyncCache = {
      ...emptyCache(),
      items: [
        shoppingItem({ id: "deleted", item: "old deleted" }),
        shoppingItem({ id: "updated", item: "old name", changeSeq: 1 }),
        shoppingItem({ id: "unchanged", item: "keep me" }),
      ],
      cursor: 40,
    };

    const next = applyShoppingListSyncResponse(
      cached,
      response({
        categoryOrder: ["Produce", "Other"],
        deleted: ["deleted"],
        changes: [
          shoppingItem({
            id: "updated",
            item: "new name",
            changeSeq: 2,
            category: "Produce",
          }),
          shoppingItem({ id: "created", item: "new item" }),
        ],
        cursor: 45,
      }),
    );

    expect(next.categoryOrder).toEqual(["Produce", "Other"]);
    expect(next.cursor).toBe(45);
    expect(next.items.map((item) => item.id).sort()).toEqual([
      "created",
      "unchanged",
      "updated",
    ]);
    expect(next.items.find((item) => item.id === "updated")).toMatchObject({
      item: "new name",
      category: "Produce",
    });
  });

  it("drops cached items a full resync omits", () => {
    const cached: ShoppingListSyncCache = {
      ...emptyCache(),
      items: [
        // Created after the restored backup was taken: the server has
        // neither the item nor a tombstone for it.
        shoppingItem({ id: "lost-in-restore", item: "lost" }),
        shoppingItem({ id: "active", item: "old active" }),
      ],
      cursor: 40,
    };

    const next = applyShoppingListSyncResponse(
      cached,
      response({
        changes: [shoppingItem({ id: "active", item: "new active" })],
        fullResync: true,
      }),
    );

    expect(next.items.map((item) => item.id)).toEqual(["active"]);
    expect(next.items[0].item).toBe("new active");
  });

  it("drops an item an op found gone", () => {
    const cached: ShoppingListSyncCache = {
      ...emptyCache(),
      items: [shoppingItem({ id: "gone", item: "deleted long ago" })],
    };
    const ops: ShoppingListOp[] = [
      { op: "update", id: "gone", fields: { isChecked: true } },
    ];

    const next = applyShoppingListSyncResponse(
      cached,
      response({ results: ["gone"] }),
      ops,
    );

    expect(next.items).toEqual([]);
  });

  it("rejects caches from before the one-protocol sync", () => {
    const storage = new MemoryStorage();
    storage.setItem(
      shoppingListCacheKey("token-a"),
      JSON.stringify({
        version: 2,
        categoryOrder: ["Other"],
        items: [],
        cursor: 40,
      }),
    );

    expect(() => loadShoppingListSyncCache(storage, "token-a")).toThrow(
      "unsupported version",
    );
  });

  it("throws on malformed cached data", () => {
    const storage = new MemoryStorage();
    storage.setItem(shoppingListCacheKey("token-a"), JSON.stringify({}));

    expect(() => loadShoppingListSyncCache(storage, "token-a")).toThrow(
      "unsupported version",
    );
  });

  it("sends ops from the cached cursor and stores the result", async () => {
    const storage = new MemoryStorage();
    const requests: unknown[] = [];
    const api = {
      syncShoppingList: async (request: unknown) => {
        requests.push(request);
        return response({
          results: requests.length === 1 ? ["applied"] : [],
          changes: [shoppingItem({ id: "new", item: "flour" })],
          cursor: 45,
        });
      },
    };
    const create: ShoppingListOp = {
      op: "create",
      id: "new",
      fields: { item: "flour" },
    };

    const first = await syncShoppingList(api, storage, "token-a", [create]);

    expect(first.results).toEqual(["applied"]);
    expect(first.cache.items.map((item) => item.item)).toEqual(["flour"]);
    expect(loadShoppingListSyncCache(storage, "token-a")).toEqual(first.cache);

    await syncShoppingList(api, storage, "token-a");
    expect(requests).toEqual([
      { syncShoppingListRequest: { cursor: undefined, ops: [create] } },
      { syncShoppingListRequest: { cursor: 45, ops: [] } },
    ]);
  });

  it("runs one sync at a time", async () => {
    const storage = new MemoryStorage();
    const order: string[] = [];
    let release: () => void = () => {};
    const api = {
      syncShoppingList: async (request: {
        syncShoppingListRequest: { cursor?: number };
      }) => {
        const label = `cursor ${request.syncShoppingListRequest.cursor}`;
        order.push(`start ${label}`);
        if (order.length === 1) {
          await new Promise<void>((resolve) => {
            release = resolve;
          });
        }
        order.push(`end ${label}`);
        return response({ cursor: 50 });
      },
    };

    const first = syncShoppingList(api, storage, "token-a");
    const second = syncShoppingList(api, storage, "token-a");
    await Promise.resolve();
    release();
    await Promise.all([first, second]);

    // The second waited for the first, and so started from its cursor.
    expect(order).toEqual([
      "start cursor undefined",
      "end cursor undefined",
      "start cursor 50",
      "end cursor 50",
    ]);
  });

  it("syncs from scratch when the cache can't be read", async () => {
    const storage = new MemoryStorage();
    storage.setItem(shoppingListCacheKey("token-a"), "not json");
    const errors: string[] = [];
    const requests: unknown[] = [];
    const api = {
      syncShoppingList: async (request: unknown) => {
        requests.push(request);
        return response({ cursor: 3 });
      },
    };

    await syncShoppingList(api, storage, "token-a", [], (context) =>
      errors.push(context),
    );

    expect(errors).toEqual(["shopping list cache read failed"]);
    expect(requests).toEqual([
      { syncShoppingListRequest: { cursor: undefined, ops: [] } },
    ]);
    expect(loadShoppingListSyncCache(storage, "token-a")?.cursor).toBe(3);
  });
});

function emptyCache(): ShoppingListSyncCache {
  return { version: 3, categoryOrder: ["Other"], items: [], cursor: null };
}

function response(
  overrides: Partial<SyncShoppingListResponse>,
): SyncShoppingListResponse {
  return {
    categoryOrder: ["Other"],
    changes: [],
    cursor: 1,
    deleted: [],
    fullResync: false,
    results: [],
    ...overrides,
  };
}

function shoppingItem(
  overrides: Partial<SyncShoppingListItem> & { id: string; item: string },
): SyncShoppingListItem {
  const { id, item, ...rest } = overrides;
  return {
    amount: undefined,
    category: "Other",
    categoryOverride: undefined,
    changeSeq: 1,
    computedCategory: "Other",
    id,
    isChecked: false,
    item,
    note: undefined,
    sortOrder: 0,
    sourceRecipeId: undefined,
    sourceRecipeTitle: undefined,
    ...rest,
  };
}
