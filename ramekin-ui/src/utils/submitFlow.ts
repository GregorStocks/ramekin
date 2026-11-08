import {
  createEffect,
  createSignal,
  onCleanup,
  untrack,
  type Accessor,
} from "solid-js";
import { extractApiError } from "./recipeFormHelpers";

/** Where an "add to ..." modal is: one of these at a time. */
export type SubmitFlowPhase =
  | { kind: "idle" }
  | { kind: "submitting" }
  | { kind: "failed"; message: string }
  | { kind: "confirmed" };

/** How long a confirmation shows before the modal closes itself. */
export const CONFIRM_DISMISS_MS = 1500;

export interface SubmitFlowOptions {
  isOpen: Accessor<boolean>;
  onClose: () => void;
  /** Reset the form for a fresh presentation. Runs untracked. */
  onOpen?: () => void;
  confirmMs?: number;
}

export interface SubmitFlow {
  phase: Accessor<SubmitFlowPhase>;
  submitting: Accessor<boolean>;
  confirmed: Accessor<boolean>;
  error: Accessor<string | null>;
  /**
   * Run `action`. It resolves to null on success, which confirms and then
   * closes after a beat, or to a message saying why the server refused.
   * A thrown error shows its API message, or `fallbackMessage`. Ignored
   * while a submission is in flight or confirmed.
   */
  submit: (
    fallbackMessage: string,
    action: () => Promise<string | null>,
  ) => Promise<void>;
  /** Close now, dropping any submission still in flight. */
  close: () => void;
}

/**
 * The submit -> confirm -> auto-close flow the "add to meal plan" and "add
 * to shopping list" modals share. Each open is its own presentation: a
 * submission or close timer from an earlier one can't touch the one that
 * is showing now.
 */
export function createSubmitFlow(options: SubmitFlowOptions): SubmitFlow {
  const confirmMs = options.confirmMs ?? CONFIRM_DISMISS_MS;
  const [phase, setPhase] = createSignal<SubmitFlowPhase>({ kind: "idle" });
  let presentation = 0;
  let closeTimer: ReturnType<typeof setTimeout> | undefined;
  let wasOpen = false;

  const clearCloseTimer = () => {
    clearTimeout(closeTimer);
    closeTimer = undefined;
  };

  const close = () => {
    presentation++;
    clearCloseTimer();
    options.onClose();
  };

  createEffect(() => {
    const open = options.isOpen();
    if (open && !wasOpen) {
      presentation++;
      clearCloseTimer();
      setPhase({ kind: "idle" });
      untrack(() => options.onOpen?.());
    }
    wasOpen = open;
  });

  onCleanup(() => {
    // A submission still in flight at unmount must not arm a close timer.
    presentation++;
    clearCloseTimer();
  });

  const submit: SubmitFlow["submit"] = async (fallbackMessage, action) => {
    const kind = phase().kind;
    if (kind === "submitting" || kind === "confirmed") return;
    const current = presentation;
    const isCurrent = () => presentation === current;
    setPhase({ kind: "submitting" });
    try {
      const refusal = await action();
      if (!isCurrent()) return;
      if (refusal) {
        setPhase({ kind: "failed", message: refusal });
        return;
      }
      setPhase({ kind: "confirmed" });
      clearCloseTimer();
      closeTimer = setTimeout(close, confirmMs);
    } catch (err) {
      const message = await extractApiError(err, fallbackMessage);
      if (isCurrent()) setPhase({ kind: "failed", message });
    }
  };

  return {
    phase,
    submitting: () => phase().kind === "submitting",
    confirmed: () => phase().kind === "confirmed",
    error: () => {
      const current = phase();
      return current.kind === "failed" ? current.message : null;
    },
    submit,
    close,
  };
}
