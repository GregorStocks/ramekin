/**
 * Monotonic request identity for "last write wins" async state.
 *
 * Start a request to get its id, then check `isCurrent(id)` after every await
 * before touching state. A newer `start()` or an `invalidate()` supersedes any
 * request already in flight, so its response — success or failure — is dropped.
 */
export function createRequestTracker() {
  let current = 0;

  return {
    start: () => {
      current += 1;
      return current;
    },
    invalidate: () => {
      current += 1;
    },
    isCurrent: (id: number) => id === current,
  };
}

export type RequestTracker = ReturnType<typeof createRequestTracker>;
