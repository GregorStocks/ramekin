import Foundation

/// How a followed scrape job ended.
enum ScrapeJobPollResult {
    /// The job saved its recipe (`recipeReady`); asked for with
    /// `untilRecipeReady`. Normalize ops may still be updating it, or one may
    /// have failed after the save.
    case recipeReady(ScrapeJobResponse)
    case completed(ScrapeJobResponse)
    case failed(ScrapeJobResponse)
}

/// Follows a scrape job until it stops. The server decides when a job has
/// stopped and when its recipe can be opened (`isTerminal`, `recipeReady`);
/// this never re-derives either from the status. Text import and proposal
/// jobs both poll through it.
struct ScrapeJobPoller {
    var getScrape: (UUID) async throws -> ScrapeJobResponse
    var intervalNanoseconds: UInt64
    /// Polls that may fail in a row (a dropped connection, a server restart)
    /// before giving up.
    var maxConsecutiveErrors = 3

    /// Polls `jobId` until it stops (or, with `untilRecipeReady`, until its
    /// recipe is saved), passing each poll to `onUpdate`. There's no
    /// deadline: the server fails a step that runs past its own. Throws
    /// `CancellationError` when the task is cancelled, or the last error once
    /// `maxConsecutiveErrors` polls in a row fail.
    @MainActor
    func poll(
        _ jobId: UUID,
        untilRecipeReady: Bool = false,
        onUpdate: (ScrapeJobResponse) -> Void = { _ in }
    ) async throws -> ScrapeJobPollResult {
        var consecutiveErrors = 0
        while true {
            try Task.checkCancellation()
            let job: ScrapeJobResponse
            do {
                job = try await getScrape(jobId)
                consecutiveErrors = 0
            } catch is CancellationError {
                throw CancellationError()
            } catch {
                consecutiveErrors += 1
                if consecutiveErrors >= maxConsecutiveErrors {
                    throw error
                }
                try await Task.sleep(nanoseconds: intervalNanoseconds)
                continue
            }
            onUpdate(job)
            if untilRecipeReady, job.recipeReady {
                return .recipeReady(job)
            }
            if job.isTerminal {
                return job.status == .completed ? .completed(job) : .failed(job)
            }
            try await Task.sleep(nanoseconds: intervalNanoseconds)
        }
    }
}
