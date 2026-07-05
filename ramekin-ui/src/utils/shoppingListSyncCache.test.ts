import { describe, expect, it } from "vitest";
import {
  applyShoppingListSyncResponse,
  loadShoppingListSyncCache,
  saveShoppingListSyncCache,
  shoppingListCacheKey,
  type ShoppingListSyncCache,
} from "./shoppingListSyncCache";
import type {
  ShoppingListItemResponse,
  SyncResponse,
  SyncServerChange,
} from "ramekin-client";

class MemoryStorage {
  private values = new Map<string, string>();

  getItem(key: string): string | null {
    return this.values.get(key) ?? null;
  }

  setItem(key: string, value: string): void {
    this.values.set(key, value);
  }
}

describe("shopping list sync cache", () => {
  it("round-trips cached items and dates through storage", () => {
    const storage = new MemoryStorage();
    const cache: ShoppingListSyncCache = {
      version: 1,
      categoryOrder: ["Produce", "Other"],
      items: [
        shoppingItem({
          id: "item-1",
          item: "apples",
          category: "Produce",
          updatedAt: new Date("2026-07-01T12:00:00.000Z"),
        }),
      ],
      lastSyncAt: new Date("2026-07-01T12:01:00.000Z"),
    };

    saveShoppingListSyncCache(storage, "token-a", cache);

    const loaded = loadShoppingListSyncCache(storage, "token-a");
    expect(loaded).toEqual(cache);
    expect(loaded?.items[0].updatedAt).toBeInstanceOf(Date);
    expect(loaded?.lastSyncAt).toBeInstanceOf(Date);
  });

  it("keeps cache entries separated by token", () => {
    const storage = new MemoryStorage();
    const cache: ShoppingListSyncCache = {
      version: 1,
      categoryOrder: ["Other"],
      items: [],
      lastSyncAt: null,
    };

    saveShoppingListSyncCache(storage, "token-a", cache);

    expect(loadShoppingListSyncCache(storage, "token-b")).toBeNull();
  });

  it("applies server changes and deletions to existing cached items", () => {
    const cached: ShoppingListSyncCache = {
      version: 1,
      categoryOrder: ["Other"],
      items: [
        shoppingItem({ id: "deleted", item: "old deleted" }),
        shoppingItem({ id: "updated", item: "old name", version: 1 }),
        shoppingItem({ id: "unchanged", item: "keep me" }),
      ],
      lastSyncAt: new Date("2026-07-01T12:00:00.000Z"),
    };
    const response: SyncResponse = {
      categoryOrder: ["Produce", "Other"],
      created: [],
      deleted: ["deleted"],
      serverChanges: [
        serverChange({
          id: "updated",
          item: "new name",
          version: 2,
          category: "Produce",
        }),
        serverChange({ id: "created", item: "new item", category: "Other" }),
      ],
      syncTimestamp: new Date("2026-07-01T12:05:00.000Z"),
      updated: [],
    };

    const next = applyShoppingListSyncResponse(cached, response);

    expect(next.categoryOrder).toEqual(["Produce", "Other"]);
    expect(next.lastSyncAt).toEqual(new Date("2026-07-01T12:05:00.000Z"));
    expect(next.items.map((item) => item.id).sort()).toEqual([
      "created",
      "unchanged",
      "updated",
    ]);
    expect(next.items.find((item) => item.id === "updated")).toMatchObject({
      item: "new name",
      version: 2,
      category: "Produce",
    });
  });

  it("throws on malformed cached data", () => {
    const storage = new MemoryStorage();
    storage.setItem(shoppingListCacheKey("token-a"), JSON.stringify({}));

    expect(() => loadShoppingListSyncCache(storage, "token-a")).toThrow(
      "unsupported version",
    );
  });
});

function shoppingItem(
  overrides: Partial<ShoppingListItemResponse> & { id: string; item: string },
): ShoppingListItemResponse {
  const { id, item, ...rest } = overrides;
  return {
    amount: undefined,
    category: "Other",
    categoryOverride: undefined,
    computedCategory: "Other",
    id,
    isChecked: false,
    item,
    note: undefined,
    sortOrder: 0,
    sourceRecipeId: undefined,
    sourceRecipeTitle: undefined,
    updatedAt: new Date("2026-07-01T12:00:00.000Z"),
    version: 1,
    ...rest,
  };
}

function serverChange(
  overrides: Partial<SyncServerChange> & { id: string; item: string },
): SyncServerChange {
  const { id, item, ...rest } = overrides;
  return {
    amount: undefined,
    category: "Other",
    categoryOverride: undefined,
    computedCategory: "Other",
    id,
    isChecked: false,
    item,
    note: undefined,
    sortOrder: 0,
    sourceRecipeId: undefined,
    sourceRecipeTitle: undefined,
    updatedAt: new Date("2026-07-01T12:00:00.000Z"),
    version: 1,
    ...rest,
  };
}
