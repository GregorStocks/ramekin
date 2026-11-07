import { describe, expect, it, vi } from "vitest";
import { createStartFlow } from "./startFlow";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe("createStartFlow", () => {
  it("sends one request when run twice while starting", async () => {
    const pending = deferred<string>();
    const start = vi.fn(() => pending.promise);
    const onStarted = vi.fn();
    const flow = createStartFlow(start, onStarted, "failed");

    const first = flow.run();
    const second = flow.run();
    expect(flow.state().kind).toBe("starting");
    expect(flow.busy()).toBe(true);

    pending.resolve("token");
    await Promise.all([first, second]);

    expect(start).toHaveBeenCalledOnce();
    expect(onStarted).toHaveBeenCalledExactlyOnceWith("token");
    expect(flow.state().kind).toBe("started");
  });

  it("does not run again once started", async () => {
    const start = vi.fn(async () => "job");
    const flow = createStartFlow(start, () => {}, "failed");

    await flow.run();
    await flow.run();

    expect(start).toHaveBeenCalledOnce();
  });

  it("reports a failure and allows a retry", async () => {
    let fail = true;
    const start = vi.fn(async () => {
      if (fail) throw new Error("boom");
      return "job";
    });
    const onStarted = vi.fn();
    const flow = createStartFlow(start, onStarted, "Could not start");

    await flow.run();
    expect(flow.state().kind).toBe("failed");
    expect(flow.error()).not.toBeNull();
    expect(flow.busy()).toBe(false);

    fail = false;
    await flow.run();

    expect(flow.error()).toBeNull();
    expect(onStarted).toHaveBeenCalledExactlyOnceWith("job");
  });
});
