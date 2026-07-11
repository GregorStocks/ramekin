import { Show } from "solid-js";
import type { Accessor } from "solid-js";

import type { CookbookBulkSelectionState } from "../../hooks/useCookbookBulkSelection";
import { AI_ENRICHMENTS } from "../../utils/aiEnrichments";

interface CookbookBulkToolbarProps {
  state: CookbookBulkSelectionState;
  total: Accessor<number>;
}

export default function CookbookBulkToolbar(props: CookbookBulkToolbarProps) {
  const state = props.state;

  return (
    <Show when={state.bulkMode()}>
      <div class="bulk-toolbar">
        <span class="bulk-count">
          {state.selected().size} selected
          <Show when={state.selectAllStatus()}>
            {" "}
            · <em>{state.selectAllStatus()}</em>
          </Show>
        </span>
        <button
          type="button"
          class="btn btn-small"
          onClick={state.selectAll}
          disabled={props.total() === 0 || state.selectAllStatus() !== null}
        >
          Select all ({props.total()})
        </button>
        <button
          type="button"
          class="btn btn-small"
          onClick={state.clearSelection}
          disabled={state.selected().size === 0}
        >
          Clear
        </button>
        <button
          type="button"
          class="btn btn-small btn-primary"
          onClick={state.openPdfExport}
          disabled={state.selected().size === 0}
        >
          Export to PDF
        </button>
        <button
          type="button"
          class="btn btn-small"
          onClick={state.bulkRescrapePhoto}
          disabled={
            state.selected().size === 0 || state.bulkProgress() !== null
          }
        >
          {state.bulkButtonLabel(
            "rescrapePhoto",
            "Rescrape photo",
            "Rescraping",
          )}
        </button>
        <button
          type="button"
          class="btn btn-small"
          onClick={state.bulkNormalizeTitle}
          disabled={
            state.selected().size === 0 || state.bulkProgress() !== null
          }
        >
          {state.bulkButtonLabel(
            "normalizeTitle",
            AI_ENRICHMENTS.normalizeTitle.bulkLabel,
            AI_ENRICHMENTS.normalizeTitle.progressVerb,
          )}
        </button>
        <button
          type="button"
          class="btn btn-small"
          onClick={state.bulkGenerateDescription}
          disabled={
            state.selected().size === 0 || state.bulkProgress() !== null
          }
        >
          {state.bulkButtonLabel(
            "description",
            AI_ENRICHMENTS.generateDescription.bulkLabel,
            AI_ENRICHMENTS.generateDescription.progressVerb,
          )}
        </button>
        <button
          type="button"
          class="btn btn-small"
          onClick={state.bulkGeneratePhoto}
          disabled={
            state.selected().size === 0 || state.bulkProgress() !== null
          }
        >
          {state.bulkButtonLabel(
            "photo",
            AI_ENRICHMENTS.generatePhoto.bulkLabel,
            AI_ENRICHMENTS.generatePhoto.progressVerb,
          )}
        </button>
      </div>
    </Show>
  );
}
