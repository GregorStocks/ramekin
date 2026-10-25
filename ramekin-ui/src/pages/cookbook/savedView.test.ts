import { describe, expect, it } from "vitest";

import { restoreCookbookView, saveCookbookView } from "./savedView";

class MemoryStorage {
  private values = new Map<string, string>();

  getItem(key: string): string | null {
    return this.values.get(key) ?? null;
  }

  setItem(key: string, value: string): void {
    this.values.set(key, value);
  }
}

describe("cookbook saved view", () => {
  it("restores nothing when nothing was saved", () => {
    expect(restoreCookbookView(new MemoryStorage(), {})).toBeNull();
  });

  it("restores the saved sort and filters but not search text", () => {
    const storage = new MemoryStorage();
    saveCookbookView(storage, "rating", 'chicken tag:dinner tag:"Quick meals"');

    expect(restoreCookbookView(storage, {})).toEqual({
      sort: "rating",
      q: 'tag:dinner tag:"Quick meals"',
    });
  });

  it("leaves params already in the URL alone", () => {
    const storage = new MemoryStorage();
    saveCookbookView(storage, "title", "has:photos");

    expect(restoreCookbookView(storage, { q: "tag:dinner" })).toEqual({
      sort: "title",
    });
    expect(restoreCookbookView(storage, { sort: "oldest" })).toEqual({
      q: "has:photos",
    });
    expect(
      restoreCookbookView(storage, { q: "tag:dinner", sort: "oldest" }),
    ).toBeNull();
  });

  it("restores nothing once the view is back to the defaults", () => {
    const storage = new MemoryStorage();
    saveCookbookView(storage, "rating", "tag:dinner");
    saveCookbookView(storage, "newest", "chicken");

    expect(restoreCookbookView(storage, {})).toBeNull();
  });
});
