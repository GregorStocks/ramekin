import { createEffect, onCleanup } from "solid-js";

const BASE_TITLE = "Ramekin";

export function formatTitle(title: string | null | undefined): string {
  return title && title.trim().length > 0
    ? `${title} · ${BASE_TITLE}`
    : BASE_TITLE;
}

/**
 * Sets document.title to the value returned by `title` and reactively updates
 * it when the accessor's value changes. Restores the base title on cleanup.
 */
export function usePageTitle(title: () => string | null | undefined): void {
  createEffect(() => {
    document.title = formatTitle(title());
  });
  onCleanup(() => {
    document.title = BASE_TITLE;
  });
}
