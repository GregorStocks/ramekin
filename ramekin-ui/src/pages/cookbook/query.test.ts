import { describe, expect, it } from "vitest";

import {
  buildQueryFromFilters,
  buildThresholdFromInput,
  countActiveFilters,
  getSortParams,
  parseQueryToFilters,
  parseSortOption,
} from "./query";
import calorieVectorsJson from "../../../../shared-test-vectors/calorie-filter.json?raw";

interface CalorieFilterVector {
  name: string;
  query: string;
  min: number | null;
  max: number | null;
  canonical: string;
}

const calorieVectors = JSON.parse(calorieVectorsJson) as CalorieFilterVector[];

describe("cookbook query filters", () => {
  it("parses text, quoted facets, aliases, thresholds, and dates", () => {
    expect(
      parseQueryToFilters(
        'weeknight "sheet pan" tag:"quick dinner" source:"NY Times" has:photo photo_size:<1000 photo_dim:>600 created:2026-01-01..2026-02-01',
      ),
    ).toEqual({
      textTerms: ["weeknight", "sheet pan"],
      filters: {
        tags: ["quick dinner"],
        source: "NY Times",
        photos: "has",
        createdAfter: "2026-01-01",
        createdBefore: "2026-02-01",
        photoSize: { op: "<", value: 1000 },
        photoDim: { op: ">", value: 600 },
        caloriesMin: "",
        caloriesMax: "",
      },
    });
  });

  it.each(calorieVectors)(
    "parses calorie filters like the server: $name",
    ({ query, min, max, canonical }) => {
      const parsed = parseQueryToFilters(query);
      const bound = (text: string) => (text === "" ? null : Number(text));
      expect(parsed.textTerms).toEqual([]);
      expect([
        bound(parsed.filters.caloriesMin),
        bound(parsed.filters.caloriesMax),
      ]).toEqual([min, max]);
      expect(buildQueryFromFilters(parsed.textTerms, parsed.filters)).toBe(
        canonical,
      );
    },
  );

  it("serializes parsed filters using the canonical query spelling", () => {
    const parsed = parseQueryToFilters(
      '"sheet pan" tag:"quick dinner" source:NYTimes no:photo created:>2026-01-01',
    );

    expect(buildQueryFromFilters(parsed.textTerms, parsed.filters)).toBe(
      '"sheet pan" tag:"quick dinner" source:NYTimes no:photos created:>2026-01-01',
    );
  });

  it("counts each active facet and each selected tag", () => {
    const { filters } = parseQueryToFilters(
      "tag:a tag:b source:test has:photos photo_size:<10 photo_dim:>20 created:<2026-01-01 calories:..500",
    );

    expect(countActiveFilters(filters)).toBe(8);
  });

  it("builds numeric thresholds only from populated numeric input", () => {
    expect(buildThresholdFromInput(" 42 ", ">")).toEqual({
      op: ">",
      value: 42,
    });
    expect(buildThresholdFromInput("", "<")).toBeNull();
    expect(buildThresholdFromInput("nope", "<")).toBeNull();
  });
});

describe("cookbook sorting", () => {
  it("defaults unknown and best URL values to newest browsing", () => {
    expect(parseSortOption("best")).toBe("newest");
    expect(parseSortOption("unknown")).toBe("newest");
  });

  it("maps display sorts to API parameters", () => {
    expect(getSortParams("best")).toEqual({});
    expect(getSortParams("newest")).toEqual({
      sortBy: "updated_at",
      sortDir: "desc",
    });
    expect(getSortParams("random")).toEqual({ sortBy: "random" });
    expect(getSortParams("calories_low")).toEqual({
      sortBy: "calories",
      sortDir: "asc",
    });
    expect(getSortParams("calories_high")).toEqual({
      sortBy: "calories",
      sortDir: "desc",
    });
    expect(parseSortOption("calories_low")).toBe("calories_low");
  });
});
