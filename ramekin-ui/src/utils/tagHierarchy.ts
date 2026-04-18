// Mirrors ramekin-core/src/tags.rs. Keep in sync manually — the list is
// short and duplication is simpler than threading a generated constant
// through the client.

export const SEEDED_NAMESPACES = [
  "ingredient",
  "course",
  "cuisine",
  "diet",
  "method",
  "season",
] as const;

export type SeededNamespace = (typeof SEEDED_NAMESPACES)[number];

export interface ParsedTag {
  namespace: string | null;
  value: string;
}

const NAMESPACE_RE = /^[a-z][a-z0-9_-]*$/;

export function parseTag(name: string): ParsedTag {
  const trimmed = name.trim();
  const parts = trimmed.split(":");
  if (parts.length !== 2) {
    return { namespace: null, value: trimmed };
  }
  const ns = parts[0].trim();
  const value = parts[1].trim();
  if (!ns || !value) {
    return { namespace: null, value: trimmed };
  }
  return { namespace: ns, value };
}

export function formatTag(namespace: string | null, value: string): string {
  const v = value.trim();
  const ns = namespace?.trim() ?? "";
  return ns ? `${ns}:${v}` : v;
}

export function isValidNamespace(namespace: string): boolean {
  return NAMESPACE_RE.test(namespace);
}

export interface NamespaceGroup {
  namespace: string | null; // null == "Uncategorized"
  tags: string[]; // raw tag names, e.g. "ingredient:chicken" or "dinner"
}

/**
 * Group tag names by namespace. Order:
 *   1. Seeded namespaces (even if empty, if `includeEmptySeeded`).
 *   2. Other user namespaces, alphabetically.
 *   3. Uncategorized, last.
 */
export function groupTags(
  names: string[],
  { includeEmptySeeded = false }: { includeEmptySeeded?: boolean } = {},
): NamespaceGroup[] {
  const buckets = new Map<string | null, string[]>();
  for (const name of names) {
    const { namespace } = parseTag(name);
    const key = namespace;
    if (!buckets.has(key)) buckets.set(key, []);
    buckets.get(key)!.push(name);
  }

  const seeded = SEEDED_NAMESPACES.filter(
    (ns) => includeEmptySeeded || buckets.has(ns),
  );
  const seededSet = new Set<string>(seeded);
  const extras = [...buckets.keys()]
    .filter((k): k is string => k !== null && !seededSet.has(k))
    .sort((a, b) => a.localeCompare(b));

  const groups: NamespaceGroup[] = [];
  for (const ns of seeded) {
    groups.push({
      namespace: ns,
      tags: (buckets.get(ns) ?? []).slice().sort(),
    });
  }
  for (const ns of extras) {
    groups.push({
      namespace: ns,
      tags: (buckets.get(ns) ?? []).slice().sort(),
    });
  }
  if (buckets.has(null)) {
    groups.push({ namespace: null, tags: buckets.get(null)!.slice().sort() });
  }
  return groups;
}

export function knownNamespaces(names: string[]): string[] {
  const found = new Set<string>(SEEDED_NAMESPACES);
  for (const n of names) {
    const p = parseTag(n);
    if (p.namespace) found.add(p.namespace);
  }
  return [...found].sort();
}
