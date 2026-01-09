import { createSignal, onMount, onCleanup, Show } from "solid-js";

interface CaptureMessage {
  type: "html";
  html: string;
  url: string;
}

interface CaptureResponse {
  recipe_id: string;
  title: string;
}

interface ErrorResponse {
  error: string;
}

type Status =
  | { type: "waiting" }
  | { type: "capturing" }
  | { type: "success"; recipeId: string; title: string }
  | { type: "error"; message: string };

export default function CapturePage() {
  const [status, setStatus] = createSignal<Status>({ type: "waiting" });

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
        const result: CaptureResponse = await response.json();
        setStatus({
          type: "success",
          recipeId: result.recipe_id,
          title: result.title,
        });
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
  });

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

      <Show when={status().type === "success"}>
        {(() => {
          const s = status();
          if (s.type !== "success") return null;
          return (
            <div class="capture-status capture-success">
              <p>Saved: {s.title}</p>
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
