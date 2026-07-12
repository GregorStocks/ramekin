import { describe, expect, it } from "vitest";
import { ErrorCode } from "ramekin-client";
import { recipeUpdateErrorMessage } from "./recipeFormHelpers";

describe("recipe update errors", () => {
  it("explains conflicts without telling the user their edits were discarded", () => {
    expect(
      recipeUpdateErrorMessage({
        code: ErrorCode.Conflict,
        message: "server conflict",
        status: 409,
      }),
    ).toBe(
      "This recipe changed since you opened it. Your edits are still here; reload before saving again.",
    );
  });

  it("uses the server message for other failures", () => {
    expect(
      recipeUpdateErrorMessage({
        code: ErrorCode.InvalidRequest,
        message: "Title cannot be empty",
        status: 400,
      }),
    ).toBe("Title cannot be empty");
  });
});
