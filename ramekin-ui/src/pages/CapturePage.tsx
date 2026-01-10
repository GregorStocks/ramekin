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

export default function CapturePage() {
  console.log("[Ramekin Capture] Component initializing");
  const [status, setStatus] = createSignal<Status>({ type: "waiting" });
  let pollInterval: ReturnType<typeof setInterval> | null = null;

  const pollJobStatus = async (jobId: string, token: string) => {
    console.log("[Ramekin Capture] Polling job status:", jobId);
    try {
      const response = await fetch(`/api/scrape/${jobId}`, {
        headers: {
          Authorization: `Bearer ${token}`,
        },
      });

      console.log("[Ramekin Capture] Poll response status:", response.status);
      if (!response.ok) {
        throw new Error(`Failed to get job status: ${response.status}`);
      }

      const job: ScrapeJobResponse = await response.json();
      console.log("[Ramekin Capture] Job status:", job.status);

      if (job.status === "completed" && job.recipe_id) {
        console.log(
          "[Ramekin Capture] Job completed, recipe_id:",
          job.recipe_id,
        );
        if (pollInterval) {
          clearInterval(pollInterval);
          pollInterval = null;
        }
        setStatus({
          type: "success",
          recipeId: job.recipe_id,
        });
      } else if (job.status === "failed") {
        console.error("[Ramekin Capture] Job failed:", job.error);
        if (pollInterval) {
          clearInterval(pollInterval);
          pollInterval = null;
        }
        setStatus({
          type: "error",
          message: job.error || "Failed to extract recipe",
        });
      } else {
        // Still processing - update status display
        setStatus({
          type: "capturing",
          jobId: job.id,
          jobStatus: job.status,
        });
      }
    } catch (err) {
      console.error("[Ramekin Capture] Poll error:", err);
      if (pollInterval) {
        clearInterval(pollInterval);
        pollInterval = null;
      }
      setStatus({
        type: "error",
        message: err instanceof Error ? err.message : "Failed to check status",
      });
    }
  };

  const handleMessage = async (event: MessageEvent) => {
    console.log(
      "[Ramekin Capture] Message received, source matches parent:",
      event.source === window.parent,
      "origin:",
      event.origin,
    );
    // Only accept messages from the parent page
    if (event.source !== window.parent) {
      console.log("[Ramekin Capture] Ignoring message from non-parent source");
      return;
    }

    const data = event.data as CaptureMessage;
    console.log("[Ramekin Capture] Message data type:", data?.type);
    if (data.type !== "html") {
      console.log("[Ramekin Capture] Ignoring non-html message");
      return;
    }

    console.log(
      "[Ramekin Capture] Received HTML, length:",
      data.html?.length,
      "url:",
      data.url,
    );
    setStatus({ type: "capturing" });

    const token = localStorage.getItem("token");
    console.log("[Ramekin Capture] Token present:", !!token);
    if (!token) {
      console.error("[Ramekin Capture] No token found in localStorage");
      setStatus({ type: "error", message: "Please log in to Ramekin first" });
      return;
    }

    try {
      console.log("[Ramekin Capture] Calling /api/scrape/capture");
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

      console.log("[Ramekin Capture] API response status:", response.status);
      if (response.ok) {
        const result: CreateScrapeResponse = await response.json();
        console.log("[Ramekin Capture] Job created, id:", result.id);
        setStatus({
          type: "capturing",
          jobId: result.id,
          jobStatus: result.status,
        });

        // Start polling for completion
        pollInterval = setInterval(() => {
          pollJobStatus(result.id, token);
        }, 500);
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
    console.log("[Ramekin Capture] onMount - checking if embedded in iframe");
    // Check if we're embedded in an iframe
    if (window.parent === window) {
      console.error(
        "[Ramekin Capture] Not in iframe (window.parent === window)",
      );
      setStatus({
        type: "error",
        message: "This page should be opened via the bookmarklet",
      });
      return;
    }
    console.log("[Ramekin Capture] Running in iframe");

    // Check if logged in
    const token = localStorage.getItem("token");
    console.log("[Ramekin Capture] Token in localStorage:", !!token);
    if (!token) {
      console.error("[Ramekin Capture] No token found - user not logged in");
      setStatus({ type: "error", message: "Please log in to Ramekin first" });
      return;
    }

    // Listen for messages from the parent
    console.log("[Ramekin Capture] Adding message listener");
    window.addEventListener("message", handleMessage);

    // Signal to the bookmarklet that we're ready
    console.log("[Ramekin Capture] Sending 'ready' message to parent");
    window.parent.postMessage("ready", "*");
    console.log("[Ramekin Capture] Ready message sent, waiting for HTML");
  });

  onCleanup(() => {
    window.removeEventListener("message", handleMessage);
    if (pollInterval) {
      clearInterval(pollInterval);
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
