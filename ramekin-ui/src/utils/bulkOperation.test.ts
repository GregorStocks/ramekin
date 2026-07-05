import { describe, expect, it } from "vitest";

import { runBulkOperation, summarizeBulkErrors } from "./bulkOperation";

describe("runBulkOperation", () => {
  it("runs every id, collects successes and failures, and reports progress", async () => {
    const progress: Array<[number, number]> = [];

    const result = await runBulkOperation({
      ids: ["recipe-a", "recipe-b", "recipe-c"],
      action: async (id) => {
        if (id === "recipe-b") throw new Error("nope");
        return `${id}-done`;
      },
      formatError: async (error) =>
        error instanceof Error ? error.message : "unknown",
      onProgress: (done, total) => progress.push([done, total]),
    });

    expect(result).toEqual({
      total: 3,
      succeeded: 2,
      results: ["recipe-a-done", "recipe-c-done"],
      errors: [{ id: "recipe-b", message: "nope" }],
    });
    expect(progress).toEqual([
      [1, 3],
      [2, 3],
      [3, 3],
    ]);
  });
});

describe("summarizeBulkErrors", () => {
  it("shows at most three shortened ids and marks additional errors", () => {
    expect(
      summarizeBulkErrors([
        { id: "0123456789", message: "one" },
        { id: "abcdefghij", message: "two" },
        { id: "klmnopqrst", message: "three" },
        { id: "uvwxyz1234", message: "four" },
      ]),
    ).toBe("01234567: one; abcdefgh: two; klmnopqr: three…");
  });
});
