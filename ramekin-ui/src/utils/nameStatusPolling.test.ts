import { describe, expect, it } from "vitest";
import { nameStatusPollDelay } from "./nameStatusPolling";

describe("nameStatusPollDelay", () => {
  it("backs off failed requests from 2 s to a minute", () => {
    expect(
      [1, 2, 3, 4, 5, 6, 7].map((failures) => nameStatusPollDelay(failures, 0)),
    ).toEqual([2000, 4000, 8000, 16000, 32000, 60000, 60000]);
  });

  it("polls every 2 s while names are pending", () => {
    expect(nameStatusPollDelay(0, 3)).toBe(2000);
  });

  it("stops when nothing is pending", () => {
    expect(nameStatusPollDelay(0, 0)).toBeNull();
  });
});
