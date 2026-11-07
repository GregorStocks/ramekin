import { createSignal } from "solid-js";
import { extractApiError } from "./recipeFormHelpers";

/**
 * A one-shot request that starts something (an import, a token mint): idle
 * until run, then starting, then either started or failed. Failed can run
 * again; starting and started can't, so a double click sends one request.
 */
export type StartFlowState =
  | { kind: "idle" }
  | { kind: "starting" }
  | { kind: "started" }
  | { kind: "failed"; message: string };

export function createStartFlow<T>(
  start: () => Promise<T>,
  onStarted: (result: T) => void,
  fallbackMessage: string,
) {
  const [state, setState] = createSignal<StartFlowState>({ kind: "idle" });

  const busy = () => {
    const kind = state().kind;
    return kind === "starting" || kind === "started";
  };

  const run = async () => {
    if (busy()) return;
    setState({ kind: "starting" });
    let result: T;
    try {
      result = await start();
    } catch (err) {
      setState({
        kind: "failed",
        message: await extractApiError(err, fallbackMessage),
      });
      return;
    }
    setState({ kind: "started" });
    onStarted(result);
  };

  const error = () => {
    const current = state();
    return current.kind === "failed" ? current.message : null;
  };

  return { state, run, busy, error };
}
