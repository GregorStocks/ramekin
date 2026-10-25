import { describe, expect, it } from "vitest";
import {
  NormalizedRecipeFromJSON,
  type NormalizeOpGroupResponse,
  type NormalizeOpInfo,
} from "ramekin-client";
import vectorsJson from "../../../shared-test-vectors/normalize-selection.json?raw";
import {
  defaultNormalizeSelection,
  effectiveNormalizeOps,
  lockedNormalizeOps,
  normalizeStepLabel,
  toggleNormalizeOp,
} from "./normalizeSelection";
import {
  normalizedRecipeChanged,
  recipeContentChanges,
} from "./recipeContentChanges";

interface VectorOp {
  id: string;
  label: string;
  group: NormalizeOpGroupResponse;
  implies: string[];
  default_on_import: boolean;
}

interface Vectors {
  ops: VectorOp[];
  default_selection: {
    name: string;
    unavailable: string[];
    expected: string[];
  }[];
  toggle: {
    name: string;
    unavailable: string[];
    selected: string[];
    toggle: string;
    expected_selected: string[];
    expected_locked: string[];
    expected_effective: string[];
  }[];
  step_labels: { step: string; expected: string }[];
  proposal_changes: {
    name: string;
    base: unknown;
    proposed: unknown;
    expected_changed: boolean;
    expected_labels: string[];
  }[];
}

const vectors = JSON.parse(vectorsJson) as Vectors;

function ops(unavailable: string[] = []): NormalizeOpInfo[] {
  return vectors.ops.map((op) => ({
    id: op.id,
    label: op.label,
    group: op.group,
    usesAi: op.group === "normalize",
    writes: [],
    runsAfter: [],
    implies: op.implies,
    defaultOnImport: op.default_on_import,
    available: !unavailable.includes(op.id),
    unavailableReason: unavailable.includes(op.id) ? "Unavailable" : null,
  }));
}

/** A set's members in registry order, for comparing with the vectors. */
function inRegistryOrder(set: Set<string>): string[] {
  return vectors.ops.map((op) => op.id).filter((id) => set.has(id));
}

describe("normalize op selection (shared vectors)", () => {
  for (const vector of vectors.default_selection) {
    it(`default selection: ${vector.name}`, () => {
      expect(
        inRegistryOrder(defaultNormalizeSelection(ops(vector.unavailable))),
      ).toEqual(vector.expected);
    });
  }

  for (const vector of vectors.toggle) {
    it(`toggle: ${vector.name}`, () => {
      const available = ops(vector.unavailable);
      const selected = toggleNormalizeOp(
        available,
        new Set(vector.selected),
        vector.toggle,
      );
      expect(inRegistryOrder(selected)).toEqual(vector.expected_selected);
      expect(inRegistryOrder(lockedNormalizeOps(available, selected))).toEqual(
        vector.expected_locked,
      );
      expect(effectiveNormalizeOps(available, selected)).toEqual(
        vector.expected_effective,
      );
    });
  }

  for (const vector of vectors.step_labels) {
    it(`step label: ${vector.step}`, () => {
      expect(normalizeStepLabel({ name: vector.step }, ops())).toBe(
        vector.expected,
      );
    });
  }

  for (const vector of vectors.proposal_changes) {
    it(`proposal changes: ${vector.name}`, () => {
      const base = NormalizedRecipeFromJSON(vector.base);
      const proposed = NormalizedRecipeFromJSON(vector.proposed);
      expect(normalizedRecipeChanged(base, proposed)).toBe(
        vector.expected_changed,
      );
      expect(
        recipeContentChanges(base.content, proposed.content).map(
          (change) => change.label,
        ),
      ).toEqual(vector.expected_labels);
    });
  }
});
