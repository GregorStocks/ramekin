import { createSignal } from "solid-js";
import { extractApiError } from "./recipeFormHelpers";
import { logger } from "./logger";

export type CaptureStatus =
  | { type: "waiting" }
  | { type: "capturing" }
  | { type: "error"; message: string };

interface CaptureMessage {
  type: "html";
  html: string;
  url: string;
}

interface CaptureFlowOptions {
  isAuthenticated: () => boolean;
  capture: (html: string, sourceUrl: string) => Promise<{ id: string }>;
  onCaptured: (jobId: string) => void;
}

/**
 * The capture iframe's state: waiting for the page's HTML, saving it, or
 * failed. Only the first "html" message while waiting starts a capture, so a
 * repeated message (the bookmarklet clicked again) can't save twice.
 */
export function createCaptureFlow(options: CaptureFlowOptions) {
  const [status, setStatus] = createSignal<CaptureStatus>({ type: "waiting" });

  const fail = (message: string) => setStatus({ type: "error", message });

  const handleData = async (data: unknown) => {
    const message = data as CaptureMessage | null;
    if (message?.type !== "html") return;
    if (status().type !== "waiting") return;

    setStatus({ type: "capturing" });

    if (!options.isAuthenticated()) {
      logger.error("Capture", "No token found in localStorage");
      fail("Please log in to Ramekin first");
      return;
    }

    try {
      const response = await options.capture(message.html, message.url);
      options.onCaptured(response.id);
    } catch (err) {
      logger.error("Capture", `API error: ${String(err)}`);
      fail(await extractApiError(err, "Failed to save recipe"));
    }
  };

  return { status, fail, handleData };
}
