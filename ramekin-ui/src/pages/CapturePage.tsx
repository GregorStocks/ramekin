import { createSignal, onMount, onCleanup, Show } from "solid-js";

interface CaptureMessage {
  type: "html";
  html: string;
  url: string;
}

interface CreateScrapeResponse {
  id: string;
  status: string;
}

interface ScrapeJobResponse {
  id: string;
  status: string;
  recipe_id?: string;
  error?: string;
}

interface ErrorResponse {
  error: string;
}

type Status =
  | { type: "waiting" }
  | { type: "capturing"; jobId?: string; jobStatus?: string }
  | { type: "success"; recipeId: string }
  | { type: "error"; message: string };

const POLL_INITIAL_MS = 500;
const POLL_MAX_MS = 5000;
const POLL_BACKOFF = 1.5;
const POLL_TIMEOUT_MS = 120_000; // 2 minutes

export default function CapturePage() {
  const [status, setStatus] = createSignal<Status>({ type: "waiting" });
  let pollTimeout: ReturnType<typeof setTimeout> | null = null;
  let pollStartTime: number | null = null;

  const pollJobStatus = async (
    jobId: string,
    token: string,
    delayMs: number,
  ) => {
    try {
      const response = await fetch(`/api/scrape/${jobId}`, {
        headers: {
          Authorization: `Bearer ${token}`,
        },
      });

      if (!response.ok) {
        throw new Error(`Failed to get job status: ${response.status}`);
      }

      const job: ScrapeJobResponse = await response.json();

      if (job.status === "completed" && job.recipe_id) {
        setStatus({
          type: "success",
          recipeId: job.recipe_id,
        });
      } else if (job.status === "failed") {
        console.error("[Ramekin Capture] Job failed:", job.error);
        setStatus({
          type: "error",
          message: job.error || "Failed to extract recipe",
        });
      } else {
        // Still processing - check timeout
        if (pollStartTime && Date.now() - pollStartTime > POLL_TIMEOUT_MS) {
          setStatus({
            type: "error",
            message: "Timed out waiting for recipe extraction",
          });
          return;
        }

        // Update status display
        setStatus({
          type: "capturing",
          jobId: job.id,
          jobStatus: job.status,
        });

        // Schedule next poll with backoff
        const nextDelay = Math.min(delayMs * POLL_BACKOFF, POLL_MAX_MS);
        pollTimeout = setTimeout(
          () => pollJobStatus(jobId, token, nextDelay),
          nextDelay,
        );
      }
    } catch (err) {
      console.error("[Ramekin Capture] Poll error:", err);
      setStatus({
        type: "error",
        message: err instanceof Error ? err.message : "Failed to check status",
      });
    }
  };

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

    const token = localStorage.getItem("token");
    if (!token) {
      console.error("[Ramekin Capture] No token found in localStorage");
      setStatus({ type: "error", message: "Please log in to Ramekin first" });
      return;
    }

    try {
      const response = await fetch("/api/scrape/capture", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          Authorization: `Bearer ${token}`,
        },
        body: JSON.stringify({
          html: data.html,
          source_url: data.url,
        }),
      });

      if (response.ok) {
        const result: CreateScrapeResponse = await response.json();
        setStatus({
          type: "capturing",
          jobId: result.id,
          jobStatus: result.status,
        });

        // Start polling with backoff
        pollStartTime = Date.now();
        pollTimeout = setTimeout(
          () => pollJobStatus(result.id, token, POLL_INITIAL_MS),
          POLL_INITIAL_MS,
        );
      } else {
        const error: ErrorResponse = await response.json();
        console.error("[Ramekin Capture] API error:", error.error);
        setStatus({ type: "error", message: error.error });
      }
    } catch (err) {
      console.error("[Ramekin Capture] Network error:", err);
      setStatus({ type: "error", message: "Network error" });
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
    const token = localStorage.getItem("token");
    if (!token) {
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
    if (pollTimeout) {
      clearTimeout(pollTimeout);
    }
  });

  const handleClose = () => {
    window.parent.postMessage({ type: "close" }, "*");
  };

  const handleViewRecipe = (recipeId: string) => {
    const recipeUrl = `${window.location.origin}/recipes/${recipeId}`;
    window.parent.postMessage({ type: "viewRecipe", url: recipeUrl }, "*");
  };

  const getStatusText = () => {
    const s = status();
    if (s.type !== "capturing") return "Saving recipe...";

    switch (s.jobStatus) {
      case "pending":
        return "Starting...";
      case "parsing":
        return "Extracting recipe...";
      default:
        return "Processing...";
    }
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
          <p>{getStatusText()}</p>
        </div>
      </Show>

      <Show when={status().type === "success"}>
        {(() => {
          const s = status();
          if (s.type !== "success") return null;
          return (
            <div class="capture-status capture-success">
              <p>Recipe saved!</p>
              <div class="capture-actions">
                <button
                  class="btn-view"
                  onClick={() => handleViewRecipe(s.recipeId)}
                >
                  View Recipe
                </button>
                <button onClick={handleClose}>Close</button>
              </div>
            </div>
          );
        })()}
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
