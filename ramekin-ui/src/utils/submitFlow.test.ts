import { createRoot, createSignal } from "solid-js";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createSubmitFlow } from "./submitFlow";

function setup() {
  return createRoot((dispose) => {
    const [isOpen, setIsOpen] = createSignal(false);
    const onClose = vi.fn(() => setIsOpen(false));
    const onOpen = vi.fn();
    const flow = createSubmitFlow({
      isOpen,
      onClose,
      onOpen,
      confirmMs: 1000,
    });
    return { flow, setIsOpen, onClose, onOpen, dispose };
  });
}

describe("createSubmitFlow", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("confirms a successful submit, then closes after a beat", async () => {
    const { flow, setIsOpen, onClose, onOpen, dispose } = setup();
    setIsOpen(true);
    expect(onOpen).toHaveBeenCalledTimes(1);

    const pending = flow.submit("Failed", async () => null);
    expect(flow.phase()).toEqual({ kind: "submitting" });
    await pending;
    expect(flow.phase()).toEqual({ kind: "confirmed" });
    expect(flow.error()).toBeNull();

    vi.advanceTimersByTime(999);
    expect(onClose).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(onClose).toHaveBeenCalledTimes(1);
    dispose();
  });

  it("shows a refusal or a thrown error, and can be retried", async () => {
    const { flow, setIsOpen, dispose } = setup();
    setIsOpen(true);

    await flow.submit("Failed", async () => "Already planned");
    expect(flow.phase()).toEqual({
      kind: "failed",
      message: "Already planned",
    });

    await flow.submit("Fallback message", async () => {
      throw new Error("boom");
    });
    expect(flow.error()).toBe("Fallback message");
    expect(flow.submitting()).toBe(false);

    await flow.submit("Failed", async () => null);
    expect(flow.confirmed()).toBe(true);
    dispose();
  });

  it("ignores a second submit while one is in flight or confirmed", async () => {
    const { flow, setIsOpen, dispose } = setup();
    setIsOpen(true);
    const action = vi.fn(async () => null);

    const first = flow.submit("Failed", action);
    await flow.submit("Failed", action);
    await first;
    await flow.submit("Failed", action);

    expect(action).toHaveBeenCalledTimes(1);
    dispose();
  });

  it("drops a submission that settles after the modal closed and reopened", async () => {
    const { flow, setIsOpen, onClose, onOpen, dispose } = setup();
    setIsOpen(true);
    let resolve!: (value: string | null) => void;
    const pending = flow.submit(
      "Failed",
      () => new Promise<string | null>((r) => (resolve = r)),
    );

    flow.close();
    setIsOpen(true);
    expect(onOpen).toHaveBeenCalledTimes(2);
    resolve(null);
    await pending;

    expect(flow.phase()).toEqual({ kind: "idle" });
    vi.advanceTimersByTime(5000);
    // Only the explicit close; no timer from the first presentation.
    expect(onClose).toHaveBeenCalledTimes(1);
    dispose();
  });

  it("doesn't arm a close timer for a submission that settles after unmount", async () => {
    const { flow, setIsOpen, onClose, dispose } = setup();
    setIsOpen(true);
    let resolve!: (value: string | null) => void;
    const pending = flow.submit(
      "Failed",
      () => new Promise<string | null>((r) => (resolve = r)),
    );

    dispose();
    resolve(null);
    await pending;
    vi.advanceTimersByTime(5000);

    expect(onClose).not.toHaveBeenCalled();
  });
});
