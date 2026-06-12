// Mirrors ramekin-ios/RamekinTests/TagHierarchySupportTests.swift where the
// logic overlaps, per doc/client-logic-sharing.md. Keep representative cases
// in sync until the shared-test-vector harness lands.
import { describe, expect, it } from "vitest";

import {
  SEEDED_NAMESPACES,
  formatTag,
  groupTags,
  isValidNamespace,
  knownNamespaces,
  parseTag,
} from "./tagHierarchy";

describe("parseTag", () => {
  it("splits hierarchical tags and trims whitespace", () => {
    expect(parseTag(" ingredient:chicken ")).toEqual({
      namespace: "ingredient",
      value: "chicken",
    });
  });

  it("treats multi-colon tags as uncategorized", () => {
    expect(parseTag("a:b:c")).toEqual({ namespace: null, value: "a:b:c" });
  });

  it("treats tags with an empty namespace or value as uncategorized", () => {
    expect(parseTag(":chicken")).toEqual({
      namespace: null,
      value: ":chicken",
    });
    expect(parseTag("ingredient:")).toEqual({
      namespace: null,
      value: "ingredient:",
    });
    expect(parseTag("dinner")).toEqual({ namespace: null, value: "dinner" });
  });
});

describe("formatTag", () => {
  it("builds hierarchical and flat tags", () => {
    expect(formatTag("ingredient", "Chicken")).toBe("ingredient:Chicken");
    expect(formatTag(null, "Dinner")).toBe("Dinner");
  });

  it("drops a blank namespace", () => {
    expect(formatTag("  ", "Dinner")).toBe("Dinner");
  });
});

describe("isValidNamespace", () => {
  it("accepts lowercase identifiers", () => {
    expect(isValidNamespace("course")).toBe(true);
    expect(isValidNamespace("side-dish_2")).toBe(true);
  });

  it("rejects invalid values", () => {
    expect(isValidNamespace("bad namespace")).toBe(false);
    expect(isValidNamespace("1course")).toBe(false);
    expect(isValidNamespace("Course")).toBe(false);
    expect(isValidNamespace("")).toBe(false);
  });
});

describe("groupTags", () => {
  it("orders seeded namespaces, then extras, then uncategorized", () => {
    const groups = groupTags([
      "season:winter",
      "occasion:holiday",
      "dinner",
      "ingredient:chicken",
    ]);

    expect(groups.map((g) => g.namespace)).toEqual([
      "ingredient",
      "season",
      "occasion",
      null,
    ]);
    expect(groups[0].tags).toEqual(["ingredient:chicken"]);
    expect(groups[3].tags).toEqual(["dinner"]);
  });

  it("sorts tags within each group", () => {
    const groups = groupTags(["course:dinner", "course:breakfast"]);
    expect(groups).toEqual([
      { namespace: "course", tags: ["course:breakfast", "course:dinner"] },
    ]);
  });

  it("includes empty seeded namespaces when requested", () => {
    const groups = groupTags(["dinner"], { includeEmptySeeded: true });
    expect(groups.map((g) => g.namespace)).toEqual([
      ...SEEDED_NAMESPACES,
      null,
    ]);
    expect(
      groups.every((g) => g.namespace === null || g.tags.length === 0),
    ).toBe(true);
  });
});

describe("knownNamespaces", () => {
  it("always includes seeded namespaces and sorts alphabetically", () => {
    expect(knownNamespaces(["occasion:holiday", "dinner"])).toEqual([
      "course",
      "cuisine",
      "diet",
      "ingredient",
      "method",
      "occasion",
      "season",
    ]);
  });
});
