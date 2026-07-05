import { describe, expect, it } from "vitest";

import vectorsJson from "../../../shared-test-vectors/scale-amount.json?raw";
import { scaleAmount } from "./scaleAmount";

type ScaleAmountVector = {
  name: string;
  amount: string;
  factor: number;
  expected: string;
};

const vectors = JSON.parse(vectorsJson) as ScaleAmountVector[];

describe("scaleAmount", () => {
  it.each(vectors)("$name", ({ amount, factor, expected }) => {
    expect(scaleAmount(amount, factor)).toBe(expected);
  });

  it("leaves amounts alone for non-finite factors", () => {
    expect(scaleAmount("1", Number.NaN)).toBe("1");
    expect(scaleAmount("1", Number.POSITIVE_INFINITY)).toBe("1");
  });

  it("returns empty string for nullish amounts", () => {
    expect(scaleAmount(null, 2)).toBe("");
    expect(scaleAmount(undefined, 2)).toBe("");
  });
});
