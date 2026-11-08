import { describe, expect, it } from "vitest";
import type { SyncApi, SyncMealPlansResponse } from "ramekin-client";
import { mealPlanOpError, writeMealPlan } from "./mealPlanWrites";

describe("writeMealPlan", () => {
  it("keeps a separate cursor for each session token", async () => {
    let nextCursor = 10;
    const sent: Array<number | null | undefined> = [];
    const api: Pick<SyncApi, "syncMealPlans"> = {
      syncMealPlans: async ({ syncMealPlansRequest }) => {
        sent.push(syncMealPlansRequest.cursor);
        return {
          cursor: nextCursor++,
          results: ["applied"],
        } as unknown as SyncMealPlansResponse;
      },
    };
    const op = { op: "delete" as const, id: crypto.randomUUID() };

    await writeMealPlan(api, "token-a", op);
    await writeMealPlan(api, "token-a", op);
    await writeMealPlan(api, "token-b", op);
    await writeMealPlan(api, "token-a", op);

    expect(sent).toEqual([undefined, 10, undefined, 11]);
  });
});

describe("mealPlanOpError", () => {
  it("explains every refused result and is null when applied", () => {
    expect(mealPlanOpError("applied", "dinner")).toBeNull();
    expect(mealPlanOpError("duplicate", "dinner")).toBe(
      "This recipe is already scheduled for dinner on this date",
    );
    expect(mealPlanOpError("recipe_missing", "lunch")).toBe(
      "This recipe no longer exists",
    );
    expect(mealPlanOpError("gone", "lunch")).toBe(
      "This meal was already removed",
    );
  });
});
