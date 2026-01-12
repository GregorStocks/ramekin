import { createSignal, Show, For } from "solid-js";
import { useAuth } from "../context/AuthContext";
import VersionSourceBadge from "./VersionSourceBadge";
import type { VersionSummary } from "ramekin-client";

interface VersionHistoryPanelProps {
  recipeId: string;
  currentVersionId: string;
  onViewVersion: (versionId: string) => void;
  onRevertVersion: (version: VersionSummary) => void;
  onCompareVersions?: (versionIds: [string, string]) => void;
}

export default function VersionHistoryPanel(props: VersionHistoryPanelProps) {
  const { getRecipesApi } = useAuth();

  const [expanded, setExpanded] = createSignal(false);
  const [versions, setVersions] = createSignal<VersionSummary[]>([]);
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [selectedForCompare, setSelectedForCompare] = createSignal<string[]>(
    [],
  );

  const toggleCompareSelection = (versionId: string) => {
    setSelectedForCompare((prev) => {
      if (prev.includes(versionId)) {
        return prev.filter((id) => id !== versionId);
      }
      if (prev.length >= 2) {
        // Replace oldest selection
        return [prev[1], versionId];
      }
      return [...prev, versionId];
    });
  };

  const canCompare = () =>
    selectedForCompare().length === 2 && props.onCompareVersions;

  const handleCompare = () => {
    const selected = selectedForCompare();
    if (selected.length === 2 && props.onCompareVersions) {
      props.onCompareVersions([selected[0], selected[1]]);
    }
  };

  const loadVersions = async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await getRecipesApi().listVersions({
        id: props.recipeId,
      });
      setVersions(response.versions);
    } catch (err) {
      setError("Failed to load version history");
    } finally {
      setLoading(false);
    }
  };

  const toggleExpanded = () => {
    const wasExpanded = expanded();
    setExpanded(!wasExpanded);
    if (!wasExpanded && versions().length === 0) {
      loadVersions();
    }
  };

  const formatDate = (date: Date) => {
    return new Intl.DateTimeFormat("en-US", {
      month: "short",
      day: "numeric",
      year: "numeric",
      hour: "numeric",
      minute: "2-digit",
    }).format(date);
  };

  return (
    <div class="version-history-panel">
      <div class="version-history-header">
        <div class="version-history-header-left" onClick={toggleExpanded}>
          <span class="version-history-toggle">{expanded() ? "▼" : "▶"}</span>
          <span>Version History</span>
          <Show when={versions().length > 0}>
            <span class="version-count">({versions().length})</span>
          </Show>
        </div>
        <Show when={canCompare()}>
          <button
            type="button"
            class="btn btn-small btn-primary"
            onClick={(e) => {
              e.stopPropagation();
              handleCompare();
            }}
          >
            Compare
          </button>
        </Show>
      </div>

      <Show when={expanded()}>
        <div class="version-list">
          <Show when={loading()}>
            <p class="loading-text">Loading versions...</p>
          </Show>

          <Show when={error()}>
            <p class="error-text">{error()}</p>
          </Show>

          <Show when={!loading() && !error()}>
            <For each={versions()}>
              {(version) => (
                <div
                  class={`version-item ${version.id === props.currentVersionId ? "current" : ""}`}
                >
                  <div class="version-item-row">
                    <Show when={props.onCompareVersions}>
                      <input
                        type="checkbox"
                        class="version-compare-checkbox"
                        checked={selectedForCompare().includes(version.id)}
                        onChange={() => toggleCompareSelection(version.id)}
                        onClick={(e) => e.stopPropagation()}
                      />
                    </Show>
                    <div class="version-item-content">
                      <div class="version-item-header">
                        <span class="version-date">
                          {formatDate(version.createdAt)}
                        </span>
                        <VersionSourceBadge source={version.versionSource} />
                        <Show when={version.isCurrent}>
                          <span class="current-badge">Current</span>
                        </Show>
                      </div>
                      <div class="version-item-title">{version.title}</div>
                      <div class="version-item-actions">
                        <button
                          type="button"
                          class="btn btn-small"
                          onClick={() => props.onViewVersion(version.id)}
                        >
                          View
                        </button>
                        <Show when={!version.isCurrent}>
                          <button
                            type="button"
                            class="btn btn-small btn-outline"
                            onClick={() => props.onRevertVersion(version)}
                          >
                            Revert
                          </button>
                        </Show>
                      </div>
                    </div>
                  </div>
                </div>
              )}
            </For>
          </Show>
        </div>
      </Show>
    </div>
  );
}
