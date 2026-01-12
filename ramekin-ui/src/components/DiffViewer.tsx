import { For } from "solid-js";
import { diffWords } from "diff";

interface DiffViewerProps {
  label: string;
  oldText: string;
  newText: string;
}

export default function DiffViewer(props: DiffViewerProps) {
  const getDiff = () => {
    return diffWords(props.oldText || "", props.newText || "");
  };

  return (
    <div class="diff-viewer">
      <div class="diff-label">{props.label}</div>
      <div class="diff-content">
        <For each={getDiff()}>
          {(part) => (
            <span
              class={
                part.added
                  ? "diff-addition"
                  : part.removed
                    ? "diff-deletion"
                    : ""
              }
            >
              {part.value}
            </span>
          )}
        </For>
      </div>
    </div>
  );
}
