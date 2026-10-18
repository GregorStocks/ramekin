import { describe, expect, it } from "vitest";

import vectorsJson from "../../../shared-test-vectors/sync-apply.json?raw";
import { applySyncResponse } from "./syncApply";

type Fields = Record<string, unknown>;

interface VectorRow {
  id: string;
  change_seq: number;
  fields: Fields;
  pending: string | null;
  dirty: string[];
}

interface Vector {
  name: string;
  cache: VectorRow[];
  response: {
    changes: { id: string; change_seq: number; fields: Fields }[];
    deleted: string[];
    full_resync: boolean;
  };
  expected: VectorRow[];
}

interface Row {
  id: string;
  changeSeq: number;
  fields: Fields;
}

const vectors = (JSON.parse(vectorsJson) as { cases: Vector[] }).cases;
// The web cache never holds unsent local writes.
const cleanCases = vectors.filter((vector) =>
  vector.cache.every((row) => row.pending === null),
);

const toRow = (row: { id: string; change_seq: number; fields: Fields }) => ({
  id: row.id,
  changeSeq: row.change_seq,
  fields: row.fields,
});

describe("sync-apply shared vectors", () => {
  it("covers some clean-cache cases", () => {
    expect(cleanCases.length).toBeGreaterThan(5);
  });

  it.each(cleanCases.map((vector) => [vector.name, vector] as const))(
    "%s",
    (_name, vector) => {
      const result: Row[] = applySyncResponse(vector.cache.map(toRow), {
        changes: vector.response.changes.map(toRow),
        deleted: vector.response.deleted,
        fullResync: vector.response.full_resync,
      });

      expect(result.sort((a, b) => a.id.localeCompare(b.id))).toEqual(
        vector.expected.map(toRow),
      );
    },
  );
});
