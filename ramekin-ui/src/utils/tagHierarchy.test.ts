import { describe, expect, it } from "vitest";

import vectorsJson from "../../../shared-test-vectors/tag-hierarchy.json?raw";
import {
  SEEDED_NAMESPACES,
  formatTag,
  groupTags,
  knownNamespaces,
  normalizeNamespace,
  parseTag,
} from "./tagHierarchy";

type TagHierarchyVectors = {
  seededNamespaces: string[];
  parseCases: Array<{
    name: string;
    input: string;
    namespace: string | null;
    value: string;
  }>;
  formatCases: Array<{
    name: string;
    namespace: string | null;
    value: string;
    expected: string;
  }>;
  normalizeNamespaceCases: Array<{
    name: string;
    input: string;
    expected: string | null;
  }>;
  groupCases: Array<{
    name: string;
    names: string[];
    expected: Array<{ namespace: string | null; names: string[] }>;
  }>;
  knownNamespacesCases: Array<{
    name: string;
    names: string[];
    expected: string[];
  }>;
};

const vectors = JSON.parse(vectorsJson) as TagHierarchyVectors;

describe("tag hierarchy shared vectors", () => {
  it("pins the seeded namespaces", () => {
    expect(SEEDED_NAMESPACES).toEqual(vectors.seededNamespaces);
  });

  it.each(vectors.parseCases)("parses $name", ({ input, namespace, value }) => {
    expect(parseTag(input)).toEqual({ namespace, value });
  });

  it.each(vectors.formatCases)(
    "formats $name",
    ({ namespace, value, expected }) => {
      expect(formatTag(namespace, value)).toBe(expected);
    },
  );

  it.each(vectors.normalizeNamespaceCases)(
    "normalizes $name",
    ({ input, expected }) => {
      expect(normalizeNamespace(input)).toBe(expected);
    },
  );

  it.each(vectors.groupCases)("groups $name", ({ names, expected }) => {
    expect(
      groupTags(names).map((group) => ({
        namespace: group.namespace,
        names: group.tags,
      })),
    ).toEqual(expected);
  });

  it.each(vectors.knownNamespacesCases)(
    "orders $name",
    ({ names, expected }) => {
      expect(knownNamespaces(names)).toEqual(expected);
    },
  );
});

describe("groupTags", () => {
  it("includes empty seeded namespaces when requested", () => {
    const groups = groupTags(["dinner"], { includeEmptySeeded: true });
    expect(groups.map((group) => group.namespace)).toEqual([
      ...SEEDED_NAMESPACES,
      null,
    ]);
  });
});
