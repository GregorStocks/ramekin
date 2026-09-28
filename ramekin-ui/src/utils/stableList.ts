/**
 * Helpers that keep refetched data from remounting the whole page.
 *
 * Solid's `<For>` keys rows by object identity, so replacing a list with a
 * freshly fetched copy tears down and recreates every row (losing focus,
 * scroll position, and re-downloading thumbnails) even when nothing changed.
 * Run each refetched list through `reuseUnchanged` so only rows whose data
 * actually changed get new identities.
 */

/** Structural equality for API response data (plain objects, arrays, Dates). */
export function deepEqual(a: unknown, b: unknown): boolean {
  if (Object.is(a, b)) return true;
  if (a instanceof Date || b instanceof Date) {
    return (
      a instanceof Date && b instanceof Date && a.getTime() === b.getTime()
    );
  }
  if (typeof a !== "object" || typeof b !== "object" || !a || !b) {
    return false;
  }
  if (Array.isArray(a) !== Array.isArray(b)) return false;
  if (Array.isArray(a) && Array.isArray(b)) {
    return a.length === b.length && a.every((v, i) => deepEqual(v, b[i]));
  }
  const aRecord = a as Record<string, unknown>;
  const bRecord = b as Record<string, unknown>;
  const aKeys = Object.keys(aRecord).filter((k) => aRecord[k] !== undefined);
  const bKeys = Object.keys(bRecord).filter((k) => bRecord[k] !== undefined);
  return (
    aKeys.length === bKeys.length &&
    aKeys.every((k) => deepEqual(aRecord[k], bRecord[k]))
  );
}

/**
 * Return `next`, substituting the previous object for every item whose key
 * matches and whose contents are unchanged. Returns `prev` itself when the
 * lists are identical, so memos downstream don't fire at all.
 */
export function reuseUnchanged<T>(
  prev: readonly T[],
  next: readonly T[],
  keyOf: (item: T) => string,
): T[] {
  const prevByKey = new Map(prev.map((item) => [keyOf(item), item] as const));
  const merged = next.map((item) => {
    const old = prevByKey.get(keyOf(item));
    return old !== undefined && deepEqual(old, item) ? old : item;
  });
  const unchanged =
    merged.length === prev.length &&
    merged.every((item, i) => item === prev[i]);
  return unchanged ? (prev as T[]) : merged;
}
