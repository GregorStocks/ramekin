import { afterEach, describe, expect, it, vi } from "vitest";

import {
  backSlotFlipAxis,
  cropRegionForAspectRatio,
  drawCutGuides,
  recipeUrlFor,
  type CutGuideDocument,
} from "./pdfExport";

class RecordingCutGuideDocument implements CutGuideDocument {
  colors: number[][] = [];
  lineWidths: number[] = [];
  lines: Array<[number, number, number, number]> = [];

  setDrawColor(...channels: number[]) {
    this.colors.push(channels);
  }

  setLineWidth(width: number) {
    this.lineWidths.push(width);
  }

  line(x1: number, y1: number, x2: number, y2: number) {
    this.lines.push([x1, y1, x2, y2]);
  }
}

describe("cropRegionForAspectRatio", () => {
  it("returns null when the image already matches the target ratio", () => {
    expect(cropRegionForAspectRatio({ pxW: 1500, pxH: 1000 }, 3 / 2)).toBe(
      null,
    );
  });

  it("crops a too-wide image from the horizontal center", () => {
    expect(cropRegionForAspectRatio({ pxW: 2000, pxH: 1000 }, 3 / 2)).toEqual({
      sourceX: 250,
      sourceY: 0,
      sourceW: 1500,
      sourceH: 1000,
    });
  });

  it("crops a too-tall image from the vertical center", () => {
    expect(cropRegionForAspectRatio({ pxW: 1000, pxH: 1000 }, 3 / 2)).toEqual({
      sourceX: 0,
      sourceY: 167,
      sourceW: 1000,
      sourceH: 667,
    });
  });

  it("leaves invalid target ratios uncropped", () => {
    expect(cropRegionForAspectRatio({ pxW: 1000, pxH: 1000 }, 0)).toBe(null);
  });
});

describe("drawCutGuides", () => {
  it("draws guide marks at each cut without crossing into the card grid", () => {
    const doc = new RecordingCutGuideDocument();

    drawCutGuides(doc, 8.5, 11, [1, 2], [3], {
      left: 0.5,
      right: 8,
      top: 0.5,
      bottom: 10,
    });

    expect(doc.colors).toEqual([[120], [0]]);
    expect(doc.lineWidths).toEqual([0.01]);
    expect(doc.lines).toEqual([
      [1, 0.08, 1, 0.3],
      [1, 10.7, 1, 10.92],
      [2, 0.08, 2, 0.3],
      [2, 10.7, 2, 10.92],
      [0.08, 3, 0.3, 3],
      [8.2, 3, 8.42, 3],
    ]);
  });

  it("skips guide marks when there is no safe page-edge space", () => {
    const doc = new RecordingCutGuideDocument();

    drawCutGuides(doc, 2, 2, [1], [1], {
      left: 0.1,
      right: 1.9,
      top: 0.1,
      bottom: 1.9,
    });

    expect(doc.lines).toEqual([]);
    expect(doc.colors).toEqual([[120], [0]]);
  });
});

describe("backSlotFlipAxis", () => {
  it("maps duplex orientation and binding to the mirrored grid axis", () => {
    expect(backSlotFlipAxis("portrait", "long-edge")).toBe("horizontal");
    expect(backSlotFlipAxis("portrait", "short-edge")).toBe("vertical");
    expect(backSlotFlipAxis("landscape", "long-edge")).toBe("vertical");
    expect(backSlotFlipAxis("landscape", "short-edge")).toBe("horizontal");
  });
});

describe("recipeUrlFor", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("builds recipe URLs from the configured external URL without trailing slashes", () => {
    vi.stubGlobal("__EXTERNAL_URL__", "https://ramekin.example///");

    expect(recipeUrlFor("recipe-1")).toBe(
      "https://ramekin.example/recipes/recipe-1",
    );
  });
});
