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
  const [status, setStatus] = createSignal<Status>({ type: "waiting" });
  let pollInterval: ReturnType<typeof setInterval> | null = null;

  const pollJobStatus = async (jobId: string, token: string) => {
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
        if (pollInterval) {
          clearInterval(pollInterval);
          pollInterval = null;
        }
        setStatus({
          type: "success",
          recipeId: job.recipe_id,
        });
      } else if (job.status === "failed") {
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
    // Only accept messages from the page that opened us
    if (event.source !== window.opener) return;

    const data = event.data as CaptureMessage;
    if (data.type !== "html") return;

    setStatus({ type: "capturing" });

    const token = localStorage.getItem("token");
    if (!token) {
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

        // Start polling for completion
        pollInterval = setInterval(() => {
          pollJobStatus(result.id, token);
        }, 500);
      } else {
        const error: ErrorResponse = await response.json();
        setStatus({ type: "error", message: error.error });
      }
    } catch {
      setStatus({ type: "error", message: "Network error" });
    }
  };

  onMount(() => {
    // Check if we were opened by a bookmarklet
    if (!window.opener) {
      setStatus({
        type: "error",
        message: "This page should be opened via the bookmarklet",
      });
      return;
    }

    // Check if logged in
    const token = localStorage.getItem("token");
    if (!token) {
      setStatus({ type: "error", message: "Please log in to Ramekin first" });
      return;
    }

    // Listen for messages from the opener
    window.addEventListener("message", handleMessage);

    // Signal to the bookmarklet that we're ready
    window.opener.postMessage("ready", "*");
  });

  onCleanup(() => {
    window.removeEventListener("message", handleMessage);
    if (pollInterval) {
      clearInterval(pollInterval);
    }
  });

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
                <a
                  href={`/recipes/${s.recipeId}`}
                  target="_blank"
                  rel="noopener"
                >
                  View Recipe
                </a>
                <button onClick={() => window.close()}>Close</button>
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
              <button onClick={() => window.close()}>Close</button>
            </div>
          );
        })()}
      </Show>
    </div>
  );
}
