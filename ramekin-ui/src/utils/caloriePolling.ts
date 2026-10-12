const FIRST_DELAY_SECONDS = 2;
const MAX_DELAY_SECONDS = 30;
const TOTAL_SECONDS = 300;

/**
 * Seconds to wait before calorie-estimate poll `index` (0-based) while names
 * are still being recognized, or null to stop: doubling from 2 s to 30 s,
 * giving up once the waits would pass 5 minutes. A stalled resolver (a
 * provider outage, the worker backing off) then costs a few requests, not one
 * every 2 s forever. Pinned with iOS by
 * shared-test-vectors/calorie-poll-delays.json.
 */
export function caloriePollDelaySeconds(index: number): number | null {
  let waited = 0;
  for (let poll = 0; poll <= index; poll++) {
    const delay = Math.min(FIRST_DELAY_SECONDS * 2 ** poll, MAX_DELAY_SECONDS);
    waited += delay;
    if (waited > TOTAL_SECONDS) return null;
    if (poll === index) return delay;
  }
  return null;
}
