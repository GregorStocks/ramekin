import { createSignal, Show, For } from "solid-js";
import { useAuth } from "../context/AuthContext";
import {
  parsePaprikaArchive,
  type ParsedPaprikaRecipe,
} from "../utils/paprikaImport";
import { usePageTitle } from "../utils/pageTitle";
import { logger } from "../utils/logger";
import { ImportExtractionMethod, type ScrapeApi } from "ramekin-client";
import {
  pollScrapeJob,
  SCRAPE_JOB_LONG_POLL_TIMEOUT_MS,
} from "../utils/pollScrapeJob";

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

type TerminalResult =
  | { status: "completed" }
  | { status: "failed"; error: string }
  | { status: "timeout" };

const IMPORT_JOB_POLL_INTERVAL_MS = 2000;

function createJobPoller(scrapeApi: ScrapeApi) {
  const waitForPollTurn = createQueuedPollTurn(IMPORT_JOB_POLL_INTERVAL_MS);

  return async (jobId: string): Promise<TerminalResult> => {
    const result = await pollScrapeJob(scrapeApi, jobId, {
      beforePoll: waitForPollTurn,
      intervalMs: 0,
      timeoutMs: SCRAPE_JOB_LONG_POLL_TIMEOUT_MS,
      onPollError: (err) => {
        logger.warn(
          "Import",
          `Error polling scrape job; retrying: ${String(err)}`,
        );
      },
    });
    if (result.status === "failed") {
      return { status: "failed", error: result.error };
    }
    return { status: result.status };
  };
}

function createQueuedPollTurn(intervalMs: number) {
  let nextTurn = Promise.resolve();
  let lastStartedAt = 0;

  return () => {
    const turn = nextTurn.then(async () => {
      const waitMs = Math.max(0, intervalMs - (Date.now() - lastStartedAt));
      if (waitMs > 0) {
        await new Promise((resolve) => setTimeout(resolve, waitMs));
      }
      lastStartedAt = Date.now();
    });
    nextTurn = turn.catch(() => {});
    return turn;
  };
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
          try {
            const terminal = await pollForJob(jobId);
            if (terminal.status === "completed") {
              updateRow(rowIndex, { state: "done", jobId });
            } else if (terminal.status === "failed") {
              updateRow(rowIndex, { state: "error", message: terminal.error });
            } else {
              updateRow(rowIndex, {
                state: "error",
                message: `Timed out waiting for import job after ${
                  SCRAPE_JOB_LONG_POLL_TIMEOUT_MS / 1000
                }s`,
              });
            }
          } catch (err) {
            updateRow(rowIndex, {
              state: "error",
              message: err instanceof Error ? err.message : String(err),
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
