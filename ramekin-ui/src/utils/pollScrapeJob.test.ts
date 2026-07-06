import { afterEach, describe, expect, it, vi } from "vitest";
import type { ScrapeApi, ScrapeJobResponse } from "ramekin-client";
import { PollScrapeJobAbortedError, pollScrapeJob } from "./pollScrapeJob";

function scrapeJob(
  status: string,
  overrides: Partial<ScrapeJobResponse> = {},
): ScrapeJobResponse {
  return {
    id: "job-id",
    url: "https://example.com/recipe",
    status,
    recipeId: null,
    error: null,
    createdAt: new Date("2026-07-06T12:00:00Z"),
    steps: [],
    ...overrides,
  } as ScrapeJobResponse;
}

function scrapeApi(responses: unknown[]): ScrapeApi {
  const getScrape = vi.fn(async () => {
    const response = responses.shift();
    if (!response) {
      throw new Error("No scrape response queued");
    }
    if (response instanceof Error || response instanceof Response) {
      throw response;
    }
    return response as ScrapeJobResponse;
  });
  return { getScrape } as unknown as ScrapeApi;
}

describe("pollScrapeJob", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("returns completed after polling non-terminal statuses", async () => {
    const api = scrapeApi([
      scrapeJob("pending"),
      scrapeJob("scraping"),
      scrapeJob("completed"),
    ]);
    const updates: string[] = [];

    const result = await pollScrapeJob(api, "job-id", {
      sleep: async () => {},
      onUpdate: (job) => updates.push(job.status),
    });

    expect(result.status).toBe("completed");
    expect(updates).toEqual(["pending", "scraping", "completed"]);
  });

  it("returns failed with the job error", async () => {
    const api = scrapeApi([scrapeJob("failed", { error: "Could not parse" })]);

    const result = await pollScrapeJob(api, "job-id");

    expect(result).toMatchObject({
      status: "failed",
      error: "Could not parse",
    });
  });

  it("returns timeout with the last observed job", async () => {
    let now = 0;
    vi.spyOn(Date, "now").mockImplementation(() => now);
    const lastJob = scrapeJob("scraping");
    const api = scrapeApi([lastJob]);

    const result = await pollScrapeJob(api, "job-id", {
      intervalMs: 50,
      timeoutMs: 100,
      sleep: async (ms) => {
        now += ms + 51;
      },
    });

    expect(result).toEqual({ status: "timeout", job: lastJob });
  });

  it("retries generated-client fetch errors", async () => {
    const fetchError = new Error("temporary failure");
    fetchError.name = "FetchError";
    const api = scrapeApi([
      scrapeJob("pending"),
      fetchError,
      scrapeJob("completed"),
    ]);
    const pollErrors: string[] = [];

    const result = await pollScrapeJob(api, "job-id", {
      sleep: async () => {},
      onPollError: (err) => {
        pollErrors.push(err instanceof Error ? err.message : String(err));
      },
    });

    expect(result.status).toBe("completed");
    expect(pollErrors).toEqual(["temporary failure"]);
  });

  it("throws plain client errors", async () => {
    const clientError = new SyntaxError("bad payload");
    const api = scrapeApi([clientError]);

    await expect(
      pollScrapeJob(api, "job-id", {
        sleep: async () => {},
      }),
    ).rejects.toBe(clientError);
  });

  it("retries 5xx scrape status responses", async () => {
    const api = scrapeApi([
      new Response("unavailable", { status: 503 }),
      scrapeJob("completed"),
    ]);

    const result = await pollScrapeJob(api, "job-id", {
      sleep: async () => {},
    });

    expect(result.status).toBe("completed");
  });

  it("throws non-transient scrape status responses", async () => {
    const notFound = new Response("not found", { status: 404 });
    const api = scrapeApi([notFound]);

    await expect(
      pollScrapeJob(api, "job-id", {
        timeoutMs: null,
        sleep: async () => {},
      }),
    ).rejects.toBe(notFound);
  });

  it("allows callers to disable timeout", async () => {
    vi.spyOn(Date, "now").mockReturnValue(1_000_000);
    const api = scrapeApi([scrapeJob("completed")]);

    const result = await pollScrapeJob(api, "job-id", {
      timeoutMs: null,
    });

    expect(result.status).toBe("completed");
  });

  it("aborts while waiting between polls", async () => {
    const controller = new AbortController();
    const api = scrapeApi([scrapeJob("pending")]);

    await expect(
      pollScrapeJob(api, "job-id", {
        signal: controller.signal,
        sleep: async () => {
          controller.abort();
        },
      }),
    ).rejects.toBeInstanceOf(PollScrapeJobAbortedError);
  });
});
