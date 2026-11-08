import { createSignal, Show, For, onCleanup } from "solid-js";
import { useAuth } from "../context/AuthContext";
import {
  parsePaprikaArchive,
  type ParsedPaprikaRecipe,
} from "../utils/paprikaImport";
import { usePageTitle } from "../utils/pageTitle";
import { logger } from "../utils/logger";
import {
  ImportExtractionMethod,
  ScrapeJobStatus,
  type ScrapeApi,
  type ScrapeJobResponse,
} from "ramekin-client";
import {
  PollScrapeJobAbortedError,
  pollScrapeJob,
} from "../utils/pollScrapeJob";

type RecipeStatus =
  | { state: "pending" }
  | { state: "importing" }
  /** The job is running; `jobStatus` is its status as last polled. */
  | { state: "running"; jobId: string; jobStatus: ScrapeJobStatus }
  | { state: "done"; jobId: string }
  | { state: "error"; message: string };

interface RecipeRow {
  name: string;
  status: RecipeStatus;
}

/** The page: waiting for a file, importing one, or unable to read it. */
type PageState =
  | { state: "idle" }
  | { state: "importing" }
  | { state: "fileError"; message: string };

type TerminalResult =
  { status: "completed" } | { status: "failed"; error: string };

const IMPORT_JOB_POLL_INTERVAL_MS = 2000;

function createJobPoller(scrapeApi: ScrapeApi, signal: AbortSignal) {
  const waitForPollTurn = createQueuedPollTurn(IMPORT_JOB_POLL_INTERVAL_MS);

  return async (
    jobId: string,
    onUpdate: (job: ScrapeJobResponse) => void,
  ): Promise<TerminalResult> => {
    const result = await pollScrapeJob(scrapeApi, jobId, {
      signal,
      beforePoll: waitForPollTurn,
      intervalMs: 0,
      // No client deadline: a job can wait a long time for a slot behind a
      // big import, and the server fails any step that runs past its own
      // deadline (with a "Timed out" error, shown on the row like any other).
      timeoutMs: null,
      onUpdate,
      onPollError: (err) => {
        logger.warn(
          "Import",
          `Error polling scrape job; retrying: ${String(err)}`,
        );
      },
    });
    switch (result.status) {
      case "completed":
        return { status: "completed" };
      case "failed":
        return { status: "failed", error: result.error };
      // Neither comes back without untilRecipeReady or a deadline.
      case "recipe_ready":
      case "timeout":
        return { status: "failed", error: "Unexpected poll result" };
    }
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

function jobStatusText(status: ScrapeJobStatus): string {
  switch (status) {
    case ScrapeJobStatus.Pending:
      return "Queued...";
    case ScrapeJobStatus.Scraping:
    case ScrapeJobStatus.Parsing:
      return "Parsing...";
    case ScrapeJobStatus.Normalizing:
      return "Saved; normalizing...";
    case ScrapeJobStatus.Completed:
      return "Finishing...";
    case ScrapeJobStatus.Failed:
      return "Failing...";
  }
}

export default function ImportPage() {
  usePageTitle(() => "Import");
  const { getPhotosApi, getImportApi, getScrapeApi } = useAuth();
  const [rows, setRows] = createSignal<RecipeRow[]>([]);
  const [page, setPage] = createSignal<PageState>({ state: "idle" });

  // Leaving the page stops this import's polling; the jobs carry on.
  let importController: AbortController | null = null;
  onCleanup(() => {
    importController?.abort();
    importController = null;
  });

  const handleFile = async (event: Event) => {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;

    importController?.abort();
    const controller = new AbortController();
    importController = controller;
    const signal = controller.signal;

    setRows([]);
    setPage({ state: "importing" });

    const failFile = (message: string) => {
      setPage({ state: "fileError", message });
      input.value = "";
    };

    let parsed: ParsedPaprikaRecipe[];
    try {
      const buf = new Uint8Array(await file.arrayBuffer());
      parsed = parsePaprikaArchive(buf);
    } catch (err) {
      if (signal.aborted) return;
      failFile(err instanceof Error ? err.message : "Failed to parse archive");
      return;
    }
    if (signal.aborted) return;

    if (parsed.length === 0) {
      failFile("No recipes found in archive");
      return;
    }

    setRows(
      parsed.map((r) => ({ name: r.name, status: { state: "pending" } })),
    );

    // Rows belong to this import: once it's aborted (the page closed or
    // another file was picked) nothing writes to them.
    const updateRow = (index: number, status: RecipeStatus) => {
      if (signal.aborted) return;
      setRows((prev) => {
        const next = prev.slice();
        next[index] = { ...next[index], status };
        return next;
      });
    };

    const photosApi = getPhotosApi();
    const importApi = getImportApi();
    const scrapeApi = getScrapeApi();
    const pollForJob = createJobPoller(scrapeApi, signal);

    const pollPromises: Promise<void>[] = [];

    for (let i = 0; i < parsed.length; i++) {
      if (signal.aborted) return;
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
        updateRow(i, { state: "running", jobId, jobStatus: response.status });
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
            const terminal = await pollForJob(jobId, (job) => {
              if (!job.isTerminal) {
                updateRow(rowIndex, {
                  state: "running",
                  jobId,
                  jobStatus: job.status,
                });
              }
            });
            if (terminal.status === "completed") {
              updateRow(rowIndex, { state: "done", jobId });
            } else {
              updateRow(rowIndex, { state: "error", message: terminal.error });
            }
          } catch (err) {
            if (err instanceof PollScrapeJobAbortedError) return;
            updateRow(rowIndex, {
              state: "error",
              message: err instanceof Error ? err.message : String(err),
            });
          }
        })(),
      );
    }

    await Promise.all(pollPromises);
    if (signal.aborted) return;
    setPage({ state: "idle" });
    input.value = "";
  };

  const statusText = (s: RecipeStatus) => {
    switch (s.state) {
      case "pending":
        return "Waiting...";
      case "importing":
        return "Importing...";
      case "running":
        return jobStatusText(s.jobStatus);
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

  const fileError = () => {
    const p = page();
    return p.state === "fileError" ? p.message : null;
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
        disabled={page().state === "importing"}
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
