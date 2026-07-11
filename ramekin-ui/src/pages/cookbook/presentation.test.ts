import { describe, expect, it } from "vitest";

import { formatRelativeDate } from "./presentation";

describe("formatRelativeDate", () => {
  const now = new Date(2026, 6, 11, 12);

  it("formats recent updates relatively", () => {
    expect(formatRelativeDate(new Date(2026, 6, 11, 8), now)).toBe(
      "Updated today",
    );
    expect(formatRelativeDate(new Date(2026, 6, 10, 8), now)).toBe(
      "Updated yesterday",
    );
    expect(formatRelativeDate(new Date(2026, 6, 7, 8), now)).toBe(
      "Updated 4 days ago",
    );
  });

  it("formats older updates as a short date", () => {
    expect(formatRelativeDate(new Date(2026, 5, 30, 8), now)).toBe(
      "Updated Jun 30",
    );
  });
});
