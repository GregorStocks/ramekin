/**
 * How long Settings waits before asking for the ingredient-recognition status
 * again, or null to stop. A failing request retries with backoff from 2 s to
 * a minute; a successful one polls every 2 s while names are still pending.
 */
export function nameStatusPollDelay(
  consecutiveFailures: number,
  pending: number,
): number | null {
  if (consecutiveFailures > 0) {
    return Math.min(2000 * 2 ** (consecutiveFailures - 1), 60000);
  }
  return pending > 0 ? 2000 : null;
}
