import { describe, expect, it } from "vitest";

import vectorsJson from "../../../shared-test-vectors/scale-amount.json?raw";
import validationJson from "../../../shared-test-vectors/recipe-scale-validation.json?raw";
import { scaleAmount, isValidRecipeScale } from "./scaleAmount";

type ScaleAmountVector = {
  name: string;
  amount: string;
  factor: number;
  expected: string;
};

const vectors = JSON.parse(vectorsJson) as ScaleAmountVector[];
const validation = JSON.parse(validationJson) as {
  name: string;
  scale: number;
  valid: boolean;
}[];

describe("scaleAmount", () => {
  it.each(validation)("validates $name scale", ({ scale, valid }) => {
    expect(isValidRecipeScale(scale)).toBe(valid);
  });
  it.each(vectors)("$name", ({ amount, factor, expected }) => {
    expect(scaleAmount(amount, factor)).toBe(expected);
  });

  it("leaves amounts alone for non-finite factors", () => {
    expect(scaleAmount("1", Number.NaN)).toBe("1");
    expect(scaleAmount("1", Number.POSITIVE_INFINITY)).toBe("1");
    expect(isValidRecipeScale(Number.NaN)).toBe(false);
    expect(isValidRecipeScale(Number.POSITIVE_INFINITY)).toBe(false);
  });

  it("returns empty string for nullish amounts", () => {
    expect(scaleAmount(null, 2)).toBe("");
    expect(scaleAmount(undefined, 2)).toBe("");
  });
});
