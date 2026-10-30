import { describe, expect, it } from "vitest";

import {
  buildQueryFromFilters,
  buildThresholdFromInput,
  countActiveFilters,
  getSortParams,
  parseQueryToFilters,
  parseSortOption,
} from "./query";

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

  it("parses and serializes calorie bounds like the server", () => {
    const bounds = (query: string) => {
      const { filters } = parseQueryToFilters(query);
      return [filters.caloriesMin, filters.caloriesMax];
    };
    expect(bounds("calories:300..600")).toEqual(["300", "600"]);
    expect(bounds("calories:300..")).toEqual(["300", ""]);
    expect(bounds("calories:..600")).toEqual(["", "600"]);
    expect(bounds("calories:>300")).toEqual(["300", ""]);
    expect(bounds("calories:<600")).toEqual(["", "600"]);
    expect(bounds("calories:lots")).toEqual(["", ""]);

    for (const [query, canonical] of [
      ["soup calories:<600", "soup calories:..600"],
      ["calories:>300", "calories:300.."],
      ["calories:300..600", "calories:300..600"],
      ["calories:lots", ""],
    ]) {
      const parsed = parseQueryToFilters(query);
      expect(buildQueryFromFilters(parsed.textTerms, parsed.filters)).toBe(
        canonical,
      );
    }
  });

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
