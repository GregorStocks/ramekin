import { beforeEach, describe, expect, it } from "vitest";
import { logger } from "./logger";

describe("logger", () => {
  beforeEach(() => {
    logger.clear();
  });

  it("records entries with level, source, and message", () => {
    logger.log("Shopping", "hello");
    logger.warn("Import", "careful");
    logger.error("Capture", "boom");

    const entries = logger.entries();
    expect(entries).toHaveLength(3);
    expect(entries[0]).toMatchObject({
      level: "log",
      source: "Shopping",
      message: "hello",
    });
    expect(entries[1].level).toBe("warn");
    expect(entries[2].level).toBe("error");
    // ISO-8601 timestamp
    expect(new Date(entries[0].timestamp).toISOString()).toBe(
      entries[0].timestamp,
    );
  });

  it("evicts the oldest entries beyond capacity", () => {
    for (let i = 0; i < 1005; i++) {
      logger.log("Test", `entry ${i}`);
    }
    const entries = logger.entries();
    expect(entries).toHaveLength(1000);
    expect(entries[0].message).toBe("entry 5");
    expect(entries[999].message).toBe("entry 1004");
  });

  it("timed logs start and completion and returns the result", async () => {
    const result = await logger.timed(
      "Shopping",
      "createItems",
      async () => 42,
    );
    expect(result).toBe(42);

    const entries = logger.entries();
    expect(entries).toHaveLength(2);
    expect(entries[0].message).toBe("createItems started");
    expect(entries[1].message).toMatch(/^createItems completed \(\d+ms\)$/);
  });

  it("timed logs failure with elapsed time and rethrows", async () => {
    await expect(
      logger.timed("Shopping", "createItems", async () => {
        throw new Error("nope");
      }),
    ).rejects.toThrow("nope");

    const entries = logger.entries();
    expect(entries).toHaveLength(2);
    expect(entries[1].level).toBe("error");
    expect(entries[1].message).toMatch(
      /^createItems FAILED after \d+ms: Error: nope$/,
    );
  });

  it("dump formats one line per entry", () => {
    logger.log("Shopping", "first");
    logger.error("Capture", "second");

    const lines = logger.dump().split("\n");
    expect(lines).toHaveLength(2);
    expect(lines[0]).toMatch(
      /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z \[log\] \[Shopping\] first$/,
    );
    expect(lines[1]).toMatch(/\[error\] \[Capture\] second$/);
  });

  it("dump returns an empty string when there are no entries", () => {
    expect(logger.dump()).toBe("");
  });
});
