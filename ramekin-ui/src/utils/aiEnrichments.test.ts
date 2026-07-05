import { describe, expect, it } from "vitest";
import {
  runRecipeAiBatchOperation,
  runSingleRecipeAiOperation,
} from "./aiEnrichments";

describe("recipe AI operations", () => {
  it("tracks progress and changed results for batch operations", async () => {
    const progress: Array<{ done: number; total: number }> = [];

    const summary = await runRecipeAiBatchOperation({
      ids: ["first-recipe-id", "second-recipe-id"],
      run: async (id) => ({ changed: id.startsWith("first") }),
      errorFallback: "failed",
      onProgress: (value) => progress.push(value),
    });

    expect(summary).toEqual({
      total: 2,
      succeeded: 2,
      changed: 1,
      errors: [],
    });
    expect(progress).toEqual([
      { done: 1, total: 2 },
      { done: 2, total: 2 },
    ]);
  });

  it("formats batch errors with recipe identifiers", async () => {
    const summary = await runRecipeAiBatchOperation({
      ids: ["1234567890abcdef"],
      run: async () => {
        throw new Response(JSON.stringify({ error: "LLM unavailable" }), {
          status: 503,
        });
      },
      errorFallback: "failed",
    });

    expect(summary).toEqual({
      total: 1,
      succeeded: 0,
      changed: 0,
      errors: ["12345678: LLM unavailable"],
    });
  });

  it("keeps single-recipe errors user-facing", async () => {
    const summary = await runSingleRecipeAiOperation(
      "1234567890abcdef",
      async () => {
        throw new Response("nope", { status: 500 });
      },
      "photo generation failed",
    );

    expect(summary.errors).toEqual(["photo generation failed (500)"]);
  });
});
