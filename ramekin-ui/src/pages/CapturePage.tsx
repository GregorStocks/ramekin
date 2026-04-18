import { createSignal, onMount, onCleanup, Show } from "solid-js";
import { useNavigate } from "@solidjs/router";
import { useAuth } from "../context/AuthContext";
import { extractApiError } from "../utils/recipeFormHelpers";
import { usePageTitle } from "../utils/pageTitle";

interface CaptureMessage {
  type: "html";
  html: string;
  url: string;
}

type Status =
  | { type: "waiting" }
  | { type: "capturing" }
  | { type: "error"; message: string };

export default function CapturePage() {
  usePageTitle(() => "Capture");
  const navigate = useNavigate();
  const { getScrapeApi, isAuthenticated } = useAuth();
  const [status, setStatus] = createSignal<Status>({ type: "waiting" });

  const handleMessage = async (event: MessageEvent) => {
    // Only accept messages from the parent page
    if (event.source !== window.parent) {
      return;
    }

    const data = event.data as CaptureMessage;
    if (data.type !== "html") {
      return;
    }

    setStatus({ type: "capturing" });

    if (!isAuthenticated()) {
      console.error("[Ramekin Capture] No token found in localStorage");
      setStatus({ type: "error", message: "Please log in to Ramekin first" });
      return;
    }

    try {
      const response = await getScrapeApi().capture({
        captureRequest: {
          html: data.html,
          sourceUrl: data.url,
        },
      });
      navigate(`/scrape/${response.id}`);
    } catch (err) {
      console.error("[Ramekin Capture] API error:", err);
      const message = await extractApiError(err, "Failed to save recipe");
      setStatus({ type: "error", message });
    }
  };

  onMount(() => {
    // Check if we're embedded in an iframe
    if (window.parent === window) {
      setStatus({
        type: "error",
        message: "This page should be opened via the bookmarklet",
      });
      return;
    }

    // Check if logged in
    if (!isAuthenticated()) {
      console.error("[Ramekin Capture] No token found - user not logged in");
      setStatus({ type: "error", message: "Please log in to Ramekin first" });
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
