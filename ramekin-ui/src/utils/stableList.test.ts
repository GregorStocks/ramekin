import { describe, expect, it } from "vitest";
import { deepEqual, reuseUnchanged } from "./stableList";

type Row = { id: string; name: string; tags?: string[]; at?: Date };

const byId = (row: Row) => row.id;

describe("deepEqual", () => {
  it("compares nested objects, arrays, and dates structurally", () => {
    expect(
      deepEqual(
        { a: [1, { b: new Date(5) }], c: undefined },
        { a: [1, { b: new Date(5) }] },
      ),
    ).toBe(true);
    expect(deepEqual({ a: new Date(5) }, { a: new Date(6) })).toBe(false);
    expect(deepEqual([1, 2], [1, 2, 3])).toBe(false);
    expect(deepEqual({ a: 1 }, { a: "1" })).toBe(false);
    expect(deepEqual(null, {})).toBe(false);
  });
});

describe("reuseUnchanged", () => {
  it("returns the previous array when nothing changed", () => {
    const prev: Row[] = [
      { id: "1", name: "a", tags: ["x"], at: new Date(1) },
      { id: "2", name: "b" },
    ];
    const next: Row[] = [
      { id: "1", name: "a", tags: ["x"], at: new Date(1) },
      { id: "2", name: "b" },
    ];
    expect(reuseUnchanged(prev, next, byId)).toBe(prev);
  });

  it("keeps identity of unchanged rows and adopts changed and new rows", () => {
    const prev: Row[] = [
      { id: "1", name: "a" },
      { id: "2", name: "b" },
    ];
    const changed = { id: "2", name: "B" };
    const added = { id: "3", name: "c" };
    const result = reuseUnchanged(
      prev,
      [added, { id: "1", name: "a" }, changed],
      byId,
    );
    expect(result).toEqual([added, prev[0], changed]);
    expect(result[1]).toBe(prev[0]);
    expect(result[2]).toBe(changed);
  });

  it("drops rows missing from the new list", () => {
    const prev: Row[] = [
      { id: "1", name: "a" },
      { id: "2", name: "b" },
    ];
    const result = reuseUnchanged(prev, [{ id: "2", name: "b" }], byId);
    expect(result).toEqual([prev[1]]);
    expect(result[0]).toBe(prev[1]);
  });
});
