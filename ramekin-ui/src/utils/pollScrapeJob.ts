import type { ScrapeApi, ScrapeJobResponse } from "ramekin-client";

export const SCRAPE_JOB_POLL_INTERVAL_MS = 500;
export const SCRAPE_JOB_POLL_TIMEOUT_MS = 120_000;
export const SCRAPE_JOB_LONG_POLL_TIMEOUT_MS = 10 * 60_000;

const TERMINAL_STATUSES = new Set(["completed", "failed"]);

export function isTerminalScrapeJobStatus(status: string): boolean {
  return TERMINAL_STATUSES.has(status);
}

export type PollScrapeJobResult =
  | { status: "completed"; job: ScrapeJobResponse }
  | { status: "failed"; job: ScrapeJobResponse; error: string }
  | { status: "timeout"; job: ScrapeJobResponse | null };

export class PollScrapeJobAbortedError extends Error {
  constructor() {
    super("Scrape job polling aborted");
    this.name = "PollScrapeJobAbortedError";
  }
}

interface PollScrapeJobOptions {
  intervalMs?: number;
  timeoutMs?: number | null;
  signal?: AbortSignal;
  onUpdate?: (job: ScrapeJobResponse) => void;
  onPollError?: (err: unknown) => void | Promise<void>;
  beforePoll?: () => Promise<void>;
  sleep?: (ms: number) => Promise<void>;
}

export async function pollScrapeJob(
  scrapeApi: ScrapeApi,
  jobId: string,
  opts: PollScrapeJobOptions = {},
): Promise<PollScrapeJobResult> {
  const intervalMs = opts.intervalMs ?? SCRAPE_JOB_POLL_INTERVAL_MS;
  const timeoutMs =
    opts.timeoutMs === undefined ? SCRAPE_JOB_POLL_TIMEOUT_MS : opts.timeoutMs;
  const startedAt = Date.now();
  let lastJob: ScrapeJobResponse | null = null;

  while (true) {
    throwIfAborted(opts.signal);
    if (timeoutMs !== null && Date.now() - startedAt > timeoutMs) {
      return { status: "timeout", job: lastJob };
    }

    try {
      await opts.beforePoll?.();
      throwIfAborted(opts.signal);
      const job = await scrapeApi.getScrape({ id: jobId });
      throwIfAborted(opts.signal);
      lastJob = job;
      opts.onUpdate?.(job);

      if (isTerminalScrapeJobStatus(job.status)) {
        if (job.status === "completed") {
          return { status: "completed", job };
        }
        return {
          status: "failed",
          job,
          error: job.error ?? "Scrape job failed",
        };
      }
    } catch (err) {
      if (err instanceof PollScrapeJobAbortedError) throw err;
      throwIfAborted(opts.signal);
      if (!isRetryablePollError(err)) {
        throw err;
      }
      await opts.onPollError?.(err);
    }

    await sleep(intervalMs, opts.signal, opts.sleep);
  }
}

function throwIfAborted(signal: AbortSignal | undefined) {
  if (signal?.aborted) {
    throw new PollScrapeJobAbortedError();
  }
}

function isRetryablePollError(err: unknown): boolean {
  const response = responseFromError(err);
  if (!response) {
    return err instanceof Error && err.name === "FetchError";
  }
  return response.status === 429 || response.status >= 500;
}

function responseFromError(err: unknown): Response | null {
  if (err instanceof Response) {
    return err;
  }
  if (
    err &&
    typeof err === "object" &&
    "response" in err &&
    err.response instanceof Response
  ) {
    return err.response;
  }
  return null;
}

async function sleep(
  ms: number,
  signal: AbortSignal | undefined,
  sleepFn: ((ms: number) => Promise<void>) | undefined,
) {
  if (sleepFn) {
    await sleepFn(ms);
    throwIfAborted(signal);
    return;
  }

  let onAbort: (() => void) | undefined;
  await new Promise<void>((resolve, reject) => {
    const timer = setTimeout(resolve, ms);
    onAbort = () => {
      clearTimeout(timer);
      reject(new PollScrapeJobAbortedError());
    };
    if (signal) {
      signal.addEventListener("abort", onAbort, { once: true });
    }
  }).finally(() => {
    if (onAbort) {
      signal?.removeEventListener("abort", onAbort);
    }
  });
}
