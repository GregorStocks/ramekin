import { describe, expect, it } from "vitest";

import vectorsJson from "../../../shared-test-vectors/meal-plan-dates.json?raw";
import { formatDateLocal, getMonday } from "./mealPlanHelpers";

type MealPlanDateVector = {
  name: string;
  year: number;
  month: number;
  day: number;
  expectedFormatted: string;
  expectedMonday: string;
};

const vectors = JSON.parse(vectorsJson) as MealPlanDateVector[];

describe("meal plan date shared vectors", () => {
  it.each(vectors)(
    "$name",
    ({ year, month, day, expectedFormatted, expectedMonday }) => {
      const date = new Date(year, month - 1, day, 12);

      expect(formatDateLocal(date)).toBe(expectedFormatted);
      expect(formatDateLocal(getMonday(date))).toBe(expectedMonday);
    },
  );
});
