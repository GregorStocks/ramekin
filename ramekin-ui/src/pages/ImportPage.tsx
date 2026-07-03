import { createSignal, Show, For } from "solid-js";
import { useAuth } from "../context/AuthContext";
import {
  parsePaprikaArchive,
  type ParsedPaprikaRecipe,
} from "../utils/paprikaImport";
import { usePageTitle } from "../utils/pageTitle";
import { logger } from "../utils/logger";
import { ImportExtractionMethod, type ScrapeApi } from "ramekin-client";

type RecipeStatus =
  | { state: "pending" }
  | { state: "importing" }
  | { state: "queued"; jobId: string }
  | { state: "done"; jobId: string }
  | { state: "error"; message: string };

interface RecipeRow {
  name: string;
  status: RecipeStatus;
}

const JOB_POLL_INTERVAL_MS = 2000;
const JOB_POLL_TIMEOUT_MS = 10 * 60 * 1000;
const TERMINAL_STATUSES = new Set(["completed", "failed"]);

type TerminalResult =
  | { status: "completed" }
  | { status: "failed"; error: string }
  | { status: "timeout" };

interface TrackedJob {
  jobId: string;
  startedAt: number;
  resolve: (result: TerminalResult) => void;
}

function createJobPoller(scrapeApi: ScrapeApi) {
  const pending: TrackedJob[] = [];
  let running = false;

  const loop = async () => {
    running = true;
    while (pending.length > 0) {
      const job = pending[0];
      if (Date.now() - job.startedAt > JOB_POLL_TIMEOUT_MS) {
        pending.shift();
        job.resolve({ status: "timeout" });
        continue;
      }
      try {
        const response = await scrapeApi.getScrape({ id: job.jobId });
        if (TERMINAL_STATUSES.has(response.status)) {
          pending.shift();
          if (response.status === "completed") {
            job.resolve({ status: "completed" });
          } else {
            job.resolve({
              status: "failed",
              error: response.error ?? "Import job failed",
            });
          }
          continue;
        }
      } catch (err) {
        logger.warn(
          "Import",
          `Error polling scrape job; retrying: ${String(err)}`,
        );
      }
      // Rotate so we don't starve other jobs behind a slow one, and wait
      // before the next request to keep pressure off /api/scrape/{id}.
      pending.push(pending.shift()!);
      await new Promise((resolve) => setTimeout(resolve, JOB_POLL_INTERVAL_MS));
    }
    running = false;
  };

  return (jobId: string): Promise<TerminalResult> =>
    new Promise((resolve) => {
      pending.push({ jobId, startedAt: Date.now(), resolve });
      if (!running) {
        void loop();
      }
    });
}

export default function ImportPage() {
  usePageTitle(() => "Import");
  const { getPhotosApi, getImportApi, getScrapeApi } = useAuth();
  const [rows, setRows] = createSignal<RecipeRow[]>([]);
  const [busy, setBusy] = createSignal(false);
  const [fileError, setFileError] = createSignal<string | null>(null);

  const handleFile = async (event: Event) => {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;

    setFileError(null);
    setRows([]);
    setBusy(true);

    let parsed: ParsedPaprikaRecipe[];
    try {
      const buf = new Uint8Array(await file.arrayBuffer());
      parsed = parsePaprikaArchive(buf);
    } catch (err) {
      setFileError(
        err instanceof Error ? err.message : "Failed to parse archive",
      );
      setBusy(false);
      input.value = "";
      return;
    }

    if (parsed.length === 0) {
      setFileError("No recipes found in archive");
      setBusy(false);
      input.value = "";
      return;
    }

    setRows(
      parsed.map((r) => ({ name: r.name, status: { state: "pending" } })),
    );

    const photosApi = getPhotosApi();
    const importApi = getImportApi();
    const scrapeApi = getScrapeApi();
    const pollForJob = createJobPoller(scrapeApi);

    const pollPromises: Promise<void>[] = [];

    for (let i = 0; i < parsed.length; i++) {
      const recipe = parsed[i];
      updateRow(i, { state: "importing" });

      let jobId: string;
      try {
        const photoIds: string[] = [];
        for (const photoBytes of recipe.photos) {
          try {
            const blob = new Blob([photoBytes as BlobPart], {
              type: "image/jpeg",
            });
            const response = await photosApi.upload({
              file: new File([blob], "photo.jpg", { type: "image/jpeg" }),
            });
            photoIds.push(response.id);
          } catch (err) {
            logger.warn(
              "Import",
              `Photo upload failed for recipe "${recipe.name}"; continuing without it: ${String(err)}`,
            );
          }
        }

        const response = await importApi.importRecipe({
          importRecipeRequest: {
            rawRecipe: recipe.rawRecipe,
            photoIds,
            extractionMethod: ImportExtractionMethod.Paprika,
          },
        });
        jobId = response.jobId;
        updateRow(i, { state: "queued", jobId });
      } catch (err) {
        updateRow(i, {
          state: "error",
          message: err instanceof Error ? err.message : String(err),
        });
        continue;
      }

      const rowIndex = i;
      pollPromises.push(
        (async () => {
          const terminal = await pollForJob(jobId);
          if (terminal.status === "completed") {
            updateRow(rowIndex, { state: "done", jobId });
          } else if (terminal.status === "failed") {
            updateRow(rowIndex, { state: "error", message: terminal.error });
          } else {
            updateRow(rowIndex, {
              state: "error",
              message: `Timed out waiting for import job after ${
                JOB_POLL_TIMEOUT_MS / 1000
              }s`,
            });
          }
        })(),
      );
    }

    await Promise.all(pollPromises);
    setBusy(false);
    input.value = "";
  };

  const updateRow = (index: number, status: RecipeStatus) => {
    setRows((prev) => {
      const next = prev.slice();
      next[index] = { ...next[index], status };
      return next;
    });
  };

  const statusText = (s: RecipeStatus) => {
    switch (s.state) {
      case "pending":
        return "Waiting...";
      case "importing":
        return "Importing...";
      case "queued":
        return "Queued...";
      case "done":
        return "Imported";
      case "error":
        return `Error: ${s.message}`;
    }
  };

  const summary = () => {
    const all = rows();
    if (all.length === 0) return null;
    const done = all.filter((r) => r.status.state === "done").length;
    const errors = all.filter((r) => r.status.state === "error").length;
    return `${done} imported, ${errors} failed, ${all.length} total`;
  };

  return (
    <div class="import-page">
      <h1>Import Paprika Recipes</h1>
      <p>
        Upload a <code>.paprikarecipes</code> file exported from Paprika. Each
        recipe is queued for import; photos are uploaded first.
      </p>
      <input
        type="file"
        accept=".paprikarecipes,application/zip"
        onChange={handleFile}
        disabled={busy()}
      />
      <Show when={fileError()}>
        <p class="error">{fileError()}</p>
      </Show>
      <Show when={rows().length > 0}>
        <p>{summary()}</p>
        <ul class="import-list">
          <For each={rows()}>
            {(row) => (
              <li>
                <strong>{row.name}</strong> — {statusText(row.status)}
              </li>
            )}
          </For>
        </ul>
      </Show>
    </div>
  );
}
