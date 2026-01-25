import {
  createSignal,
  createResource,
  For,
  Show,
  onMount,
  onCleanup,
} from "solid-js";
import { useAuth } from "../context/AuthContext";
import type { EnrichmentInfo } from "ramekin-client";

interface EnrichmentDropdownProps {
  onSelect: (enrichmentType: string) => void;
  disabled?: boolean;
  loading?: boolean;
}

export default function EnrichmentDropdown(props: EnrichmentDropdownProps) {
  const { getEnrichApi } = useAuth();
  const [isOpen, setIsOpen] = createSignal(false);

  // Fetch available enrichments
  const [enrichments] = createResource(async () => {
    try {
      const response = await getEnrichApi().listEnrichments();
      return response.enrichments;
    } catch (err) {
      console.error("Failed to fetch enrichments:", err);
      return [];
    }
  });

  const handleSelect = (enrichment: EnrichmentInfo) => {
    setIsOpen(false);
    props.onSelect(enrichment.type);
  };

  const toggleDropdown = () => {
    if (!props.disabled && !props.loading) {
      setIsOpen(!isOpen());
    }
  };

  // Close dropdown when clicking outside
  const handleClickOutside = (e: MouseEvent) => {
    const target = e.target as HTMLElement;
    if (!target.closest(".enrichment-dropdown")) {
      setIsOpen(false);
    }
  };

  // Add/remove click listener with proper cleanup
  onMount(() => {
    document.addEventListener("click", handleClickOutside);
  });

  onCleanup(() => {
    document.removeEventListener("click", handleClickOutside);
  });

  return (
    <div class="enrichment-dropdown">
      <button
        type="button"
        class="btn enrichment-dropdown-toggle"
        onClick={toggleDropdown}
        disabled={props.disabled || props.loading}
      >
        <Show when={props.loading} fallback="Enrich">
          Enriching...
        </Show>
        <span class="enrichment-dropdown-arrow">
          {isOpen() ? "\u25B2" : "\u25BC"}
        </span>
      </button>

      <Show when={isOpen()}>
        <div class="enrichment-dropdown-menu">
          <Show
            when={!enrichments.loading && enrichments()}
            fallback={<div class="enrichment-dropdown-loading">Loading...</div>}
          >
            <For each={enrichments()}>
              {(enrichment) => (
                <button
                  type="button"
                  class="enrichment-dropdown-item"
                  onClick={() => handleSelect(enrichment)}
                >
                  <span class="enrichment-dropdown-item-name">
                    {enrichment.displayName}
                  </span>
                  <span class="enrichment-dropdown-item-desc">
                    {enrichment.description}
                  </span>
                </button>
              )}
            </For>
          </Show>
        </div>
      </Show>
    </div>
  );
}
