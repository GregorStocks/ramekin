import { onMount, onCleanup, Show } from "solid-js";
import { useNavigate } from "@solidjs/router";
import { useAuth } from "../context/AuthContext";
import { usePageTitle } from "../utils/pageTitle";
import { logger } from "../utils/logger";
import { createCaptureFlow } from "../utils/captureFlow";

export default function CapturePage() {
  usePageTitle(() => "Capture");
  const navigate = useNavigate();
  const { getScrapeApi, isAuthenticated } = useAuth();
  const flow = createCaptureFlow({
    isAuthenticated,
    capture: (html, sourceUrl) =>
      getScrapeApi().capture({ captureRequest: { html, sourceUrl } }),
    onCaptured: (jobId) => navigate(`/scrape/${jobId}`),
  });
  const status = flow.status;

  const handleMessage = (event: MessageEvent) => {
    // Only accept messages from the parent page
    if (event.source !== window.parent) {
      return;
    }
    void flow.handleData(event.data);
  };

  onMount(() => {
    // Check if we're embedded in an iframe
    if (window.parent === window) {
      flow.fail("This page should be opened via the bookmarklet");
      return;
    }

    // Check if logged in
    if (!isAuthenticated()) {
      logger.error("Capture", "No token found - user not logged in");
      flow.fail("Please log in to Ramekin first");
      return;
    }

    // Listen for messages from the parent
    window.addEventListener("message", handleMessage);

    // Signal to the bookmarklet that we're ready
    window.parent.postMessage("ready", "*");
  });

  onCleanup(() => {
    window.removeEventListener("message", handleMessage);
  });

  const handleClose = () => {
    window.parent.postMessage({ type: "close" }, "*");
  };

  return (
    <div class="capture-page">
      <Show when={status().type === "waiting"}>
        <div class="capture-status">
          <div class="spinner" />
          <p>Waiting for recipe...</p>
        </div>
      </Show>

      <Show when={status().type === "capturing"}>
        <div class="capture-status">
          <div class="spinner" />
          <p>Saving recipe...</p>
        </div>
      </Show>

      <Show when={status().type === "error"}>
        {(() => {
          const s = status();
          if (s.type !== "error") return null;
          return (
            <div class="capture-status capture-error">
              <p>{s.message}</p>
              <button onClick={handleClose}>Close</button>
            </div>
          );
        })()}
      </Show>
    </div>
  );
}
