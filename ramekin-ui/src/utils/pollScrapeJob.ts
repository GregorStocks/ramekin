import type { ScrapeApi, ScrapeJobResponse } from "ramekin-client";

export const SCRAPE_JOB_POLL_INTERVAL_MS = 500;
export const SCRAPE_JOB_POLL_TIMEOUT_MS = 120_000;

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
  timeoutMs?: number;
  signal?: AbortSignal;
  onUpdate?: (job: ScrapeJobResponse) => void;
  sleep?: (ms: number) => Promise<void>;
}

export async function pollScrapeJob(
  scrapeApi: ScrapeApi,
  jobId: string,
  opts: PollScrapeJobOptions = {},
): Promise<PollScrapeJobResult> {
  const intervalMs = opts.intervalMs ?? SCRAPE_JOB_POLL_INTERVAL_MS;
  const timeoutMs = opts.timeoutMs ?? SCRAPE_JOB_POLL_TIMEOUT_MS;
  const startedAt = Date.now();
  let lastJob: ScrapeJobResponse | null = null;

  while (true) {
    throwIfAborted(opts.signal);
    if (Date.now() - startedAt > timeoutMs) {
      return { status: "timeout", job: lastJob };
    }

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

    await sleep(intervalMs, opts.signal, opts.sleep);
  }
}

function throwIfAborted(signal: AbortSignal | undefined) {
  if (signal?.aborted) {
    throw new PollScrapeJobAbortedError();
  }
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
