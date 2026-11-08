import { describe, expect, it, vi } from "vitest";
import { createCaptureFlow } from "./captureFlow";

const htmlMessage = {
  type: "html",
  html: "<html></html>",
  url: "https://example.com/r",
};

describe("createCaptureFlow", () => {
  it("captures once when the page posts its HTML twice", async () => {
    let resolveCapture!: (value: { id: string }) => void;
    const capture = vi.fn(
      () =>
        new Promise<{ id: string }>((resolve) => {
          resolveCapture = resolve;
        }),
    );
    const onCaptured = vi.fn();
    const flow = createCaptureFlow({
      isAuthenticated: () => true,
      capture,
      onCaptured,
    });

    const first = flow.handleData(htmlMessage);
    const second = flow.handleData(htmlMessage);
    resolveCapture({ id: "job-1" });
    await Promise.all([first, second]);

    expect(capture).toHaveBeenCalledOnce();
    expect(onCaptured).toHaveBeenCalledExactlyOnceWith("job-1");
  });

  it("ignores messages after a failure", async () => {
    const capture = vi.fn(async () => {
      throw new Error("boom");
    });
    const flow = createCaptureFlow({
      isAuthenticated: () => true,
      capture,
      onCaptured: () => {},
    });

    await flow.handleData(htmlMessage);
    expect(flow.status().type).toBe("error");

    await flow.handleData(htmlMessage);
    expect(capture).toHaveBeenCalledOnce();
  });

  it("ignores messages that are not page HTML", async () => {
    const capture = vi.fn(async () => ({ id: "job" }));
    const flow = createCaptureFlow({
      isAuthenticated: () => true,
      capture,
      onCaptured: () => {},
    });

    await flow.handleData("ready");
    await flow.handleData(null);
    await flow.handleData({ type: "close" });

    expect(capture).not.toHaveBeenCalled();
    expect(flow.status().type).toBe("waiting");
  });

  it("fails without capturing when logged out", async () => {
    const capture = vi.fn(async () => ({ id: "job" }));
    const flow = createCaptureFlow({
      isAuthenticated: () => false,
      capture,
      onCaptured: () => {},
    });

    await flow.handleData(htmlMessage);

    expect(capture).not.toHaveBeenCalled();
    expect(flow.status()).toEqual({
      type: "error",
      message: "Please log in to Ramekin first",
    });
  });
});
