// Mirrors ramekin-ios/RamekinTests/RecipeScaleSupportTests.swift where the
// logic overlaps, per doc/client-logic-sharing.md. Keep representative cases
// in sync until the shared-test-vector harness lands.
import { describe, expect, it } from "vitest";

import { scaleAmount } from "./scaleAmount";

describe("scaleAmount", () => {
  it("doubles representative amounts", () => {
    expect(scaleAmount("1", 2)).toBe("2");
    expect(scaleAmount("1.5", 2)).toBe("3");
    expect(scaleAmount("1/2", 2)).toBe("1");
    expect(scaleAmount("1 1/2", 2)).toBe("3");
    expect(scaleAmount(".5", 2)).toBe("1");
  });

  it("halves representative amounts", () => {
    expect(scaleAmount("1", 0.5)).toBe("1/2");
    expect(scaleAmount("2", 0.5)).toBe("1");
    expect(scaleAmount("1/2", 0.5)).toBe("1/4");
    expect(scaleAmount("1 1/2", 0.5)).toBe("0.75");
  });

  it("normalizes unicode fractions before scaling", () => {
    expect(scaleAmount("½", 2)).toBe("1");
    expect(scaleAmount("1½", 2)).toBe("3");
    expect(scaleAmount("¼", 0.5)).toBe("1/8");
  });

  it("formats non-unit fractions as trimmed decimals", () => {
    expect(scaleAmount("1/3", 2)).toBe("0.67");
    expect(scaleAmount("1", 1.5)).toBe("1.5");
    expect(scaleAmount("1/6", 2)).toBe("1/3");
  });

  it("leaves unparseable amounts alone", () => {
    expect(scaleAmount("1-2", 2)).toBe("1-2");
    expect(scaleAmount("to taste", 2)).toBe("to taste");
    expect(scaleAmount("1/0", 2)).toBe("1/0");
  });

  it("leaves amounts alone for non-scaling factors", () => {
    expect(scaleAmount("1", 1)).toBe("1");
    expect(scaleAmount("1", 0)).toBe("1");
    expect(scaleAmount("1", -2)).toBe("1");
    expect(scaleAmount("1", Number.NaN)).toBe("1");
    expect(scaleAmount("1", Number.POSITIVE_INFINITY)).toBe("1");
  });

  it("returns empty string for missing amounts", () => {
    expect(scaleAmount("", 2)).toBe("");
    expect(scaleAmount(null, 2)).toBe("");
    expect(scaleAmount(undefined, 2)).toBe("");
  });
});
