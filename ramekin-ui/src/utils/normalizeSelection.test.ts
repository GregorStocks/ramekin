import { describe, expect, it } from "vitest";
import type { NormalizeOpInfo } from "ramekin-client";
import {
  defaultNormalizeSelection,
  effectiveNormalizeOps,
  lockedNormalizeOps,
  normalizeStepLabel,
  toggleNormalizeOp,
} from "./normalizeSelection";

// Mirrored in ramekin-ios/RamekinTests/NormalizeSelectionTests.swift.

function op(
  id: string,
  overrides: Partial<NormalizeOpInfo> = {},
): NormalizeOpInfo {
  return {
    id,
    label: id,
    group: "normalize",
    usesAi: true,
    writes: [],
    runsAfter: [],
    implies: [],
    defaultOnImport: true,
    available: true,
    ...overrides,
  };
}

const OPS: NormalizeOpInfo[] = [
  op("refetch", {
    label: "Re-fetch from source",
    group: "source",
    usesAi: false,
    implies: ["reextract"],
    defaultOnImport: false,
  }),
  op("reextract", {
    label: "Re-extract from saved source",
    group: "source",
    usesAi: false,
    defaultOnImport: false,
  }),
  op("normalize_title", { label: "Normalize title" }),
  op("generate_description", { label: "Generate description" }),
  op("auto_tag", { label: "Auto-tag" }),
];

describe("normalize op selection", () => {
  it("starts with the ops imports run", () => {
    expect([...defaultNormalizeSelection(OPS)]).toEqual([
      "normalize_title",
      "generate_description",
      "auto_tag",
    ]);
  });

  it("leaves unavailable default ops unchecked", () => {
    const ops = OPS.map((o) =>
      o.id === "auto_tag" ? { ...o, available: false } : o,
    );
    expect(defaultNormalizeSelection(ops).has("auto_tag")).toBe(false);
  });

  it("checking re-fetch checks and locks re-extract", () => {
    const selected = toggleNormalizeOp(OPS, new Set(), "refetch");
    expect([...selected]).toEqual(["refetch"]);
    expect(lockedNormalizeOps(OPS, selected).has("reextract")).toBe(true);
    expect(effectiveNormalizeOps(OPS, selected)).toEqual([
      "refetch",
      "reextract",
    ]);
    // Re-extract can't be unchecked while re-fetch is checked.
    expect(toggleNormalizeOp(OPS, selected, "reextract")).toBe(selected);
  });

  it("unchecking re-fetch unlocks re-extract", () => {
    const selected = toggleNormalizeOp(OPS, new Set(["refetch"]), "refetch");
    expect(selected.size).toBe(0);
    expect(effectiveNormalizeOps(OPS, selected)).toEqual([]);
  });

  it("does not check an unavailable op", () => {
    const ops = OPS.map((o) =>
      o.id === "refetch"
        ? { ...o, available: false, unavailableReason: "No source URL" }
        : o,
    );
    const selected = new Set<string>();
    expect(toggleNormalizeOp(ops, selected, "refetch")).toBe(selected);
  });

  it("lists effective ops in registry order", () => {
    expect(
      effectiveNormalizeOps(OPS, new Set(["auto_tag", "normalize_title"])),
    ).toEqual(["normalize_title", "auto_tag"]);
  });

  it("labels op steps by their op and other steps by what they do", () => {
    expect(normalizeStepLabel({ name: "normalize_title" }, OPS)).toBe(
      "Normalize title",
    );
    expect(normalizeStepLabel({ name: "fetch_html" }, OPS)).toBe(
      "Fetching the page",
    );
    expect(normalizeStepLabel({ name: "some_new_step" }, OPS)).toBe(
      "some new step",
    );
  });
});
