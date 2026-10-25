import type { NormalizeOpInfo, StepState } from "ramekin-client";

/**
 * Which ops the Re-normalize sheet starts with checked: the ones imports run,
 * where the recipe can run them.
 *
 * Mirrored in ramekin-ios/Ramekin/NormalizeSelection.swift and pinned by
 * shared-test-vectors/normalize-selection.json.
 */
export function defaultNormalizeSelection(ops: NormalizeOpInfo[]): Set<string> {
  return new Set(
    ops.filter((op) => op.defaultOnImport && op.available).map((op) => op.id),
  );
}

/** Ops some checked op implies, which stay checked and can't be unchecked. */
export function lockedNormalizeOps(
  ops: NormalizeOpInfo[],
  selected: Set<string>,
): Set<string> {
  const locked = new Set<string>();
  for (const op of ops) {
    if (selected.has(op.id)) op.implies.forEach((id) => locked.add(id));
  }
  return locked;
}

/** The checked ops plus the ones they imply, in the registry's run order. */
export function effectiveNormalizeOps(
  ops: NormalizeOpInfo[],
  selected: Set<string>,
): string[] {
  const locked = lockedNormalizeOps(ops, selected);
  return ops
    .filter((op) => selected.has(op.id) || locked.has(op.id))
    .map((op) => op.id);
}

/**
 * Check or uncheck an op. Unavailable ops and ops locked by an implying op
 * don't change.
 */
export function toggleNormalizeOp(
  ops: NormalizeOpInfo[],
  selected: Set<string>,
  id: string,
): Set<string> {
  const op = ops.find((candidate) => candidate.id === id);
  if (!op || !op.available || lockedNormalizeOps(ops, selected).has(id)) {
    return selected;
  }
  const next = new Set(selected);
  if (next.has(id)) next.delete(id);
  else next.add(id);
  return next;
}

const STEP_LABELS: Record<string, string> = {
  fetch_html: "Fetching the page",
  extract_recipe: "Extracting the recipe",
  parse_ingredients: "Parsing ingredients",
  fetch_images: "Fetching photos",
  load_recipe: "Loading the recipe",
  resolve_ingredient_names: "Matching ingredients",
  propose_normalize: "Preparing the preview",
};

/** A job step's name as the progress list shows it. */
export function normalizeStepLabel(
  step: Pick<StepState, "name">,
  ops: NormalizeOpInfo[],
): string {
  return (
    ops.find((op) => op.id === step.name)?.label ??
    STEP_LABELS[step.name] ??
    step.name.replaceAll("_", " ")
  );
}
