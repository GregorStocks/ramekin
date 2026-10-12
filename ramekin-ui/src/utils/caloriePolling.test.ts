import { describe, expect, it } from "vitest";

import vectorsJson from "../../../shared-test-vectors/calorie-poll-delays.json?raw";
import { caloriePollDelaySeconds } from "./caloriePolling";

const vectors = JSON.parse(vectorsJson) as { delaysSeconds: number[] };

describe("caloriePollDelaySeconds", () => {
  it("matches the shared schedule and then stops", () => {
    const delays = vectors.delaysSeconds.map((_, index) =>
      caloriePollDelaySeconds(index),
    );
    expect(delays).toEqual(vectors.delaysSeconds);
    expect(caloriePollDelaySeconds(vectors.delaysSeconds.length)).toBeNull();
    expect(caloriePollDelaySeconds(1000)).toBeNull();
  });
});
