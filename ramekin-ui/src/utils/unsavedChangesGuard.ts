import { onCleanup, onMount } from "solid-js";
import { useBeforeLeave } from "@solidjs/router";

export const DISCARD_CHANGES_PROMPT =
  "Discard your unsaved changes? This can't be undone.";

/**
 * Ask before leaving a page that holds unsaved work: in-app navigation
 * (Cancel, links, Back) gets a confirm, and closing or reloading the tab
 * gets the browser's own prompt.
 */
export function useUnsavedChangesGuard(hasUnsavedChanges: () => boolean) {
  useBeforeLeave((e) => {
    if (e.defaultPrevented || !hasUnsavedChanges()) return;
    e.preventDefault();
    // Defer so the router finishes handling the blocked navigation first.
    setTimeout(() => {
      if (window.confirm(DISCARD_CHANGES_PROMPT)) e.retry(true);
    }, 0);
  });

  const handleBeforeUnload = (e: BeforeUnloadEvent) => {
    if (!hasUnsavedChanges()) return;
    e.preventDefault();
    e.returnValue = "";
  };
  onMount(() => window.addEventListener("beforeunload", handleBeforeUnload));
  onCleanup(() =>
    window.removeEventListener("beforeunload", handleBeforeUnload),
  );
}
