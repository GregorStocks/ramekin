import { For, Show } from "solid-js";
import type { NormalizeOpInfo } from "ramekin-client";
import { lockedNormalizeOps } from "../utils/normalizeSelection";

interface NormalizeOpPickerProps {
  ops: NormalizeOpInfo[];
  selected: Set<string>;
  onToggle: (id: string) => void;
  disabled?: boolean;
}

const GROUPS = [
  { group: "source", label: "Source" },
  { group: "normalize", label: "Normalize" },
] as const;

/** The Re-normalize sheet's checklist of ops, grouped as the registry groups them. */
export default function NormalizeOpPicker(props: NormalizeOpPickerProps) {
  const locked = () => lockedNormalizeOps(props.ops, props.selected);
  const implyingLabel = (id: string) =>
    props.ops
      .filter((op) => props.selected.has(op.id) && op.implies.includes(id))
      .map((op) => op.label)
      .join(", ");

  return (
    <div class="normalize-op-picker">
      <For each={GROUPS}>
        {({ group, label }) => (
          <Show when={props.ops.some((op) => op.group === group)}>
            <fieldset class="normalize-op-group">
              <legend>{label}</legend>
              <For each={props.ops.filter((op) => op.group === group)}>
                {(op) => (
                  <label
                    class="normalize-op"
                    classList={{ "normalize-op-unavailable": !op.available }}
                  >
                    <input
                      type="checkbox"
                      checked={props.selected.has(op.id) || locked().has(op.id)}
                      disabled={
                        props.disabled || !op.available || locked().has(op.id)
                      }
                      onChange={() => props.onToggle(op.id)}
                    />
                    <span class="normalize-op-text">
                      <span>{op.label}</span>
                      <Show when={!op.available && op.unavailableReason}>
                        <span class="normalize-op-note">
                          {op.unavailableReason}
                        </span>
                      </Show>
                      <Show when={locked().has(op.id)}>
                        <span class="normalize-op-note">
                          Included with {implyingLabel(op.id)}
                        </span>
                      </Show>
                    </span>
                  </label>
                )}
              </For>
            </fieldset>
          </Show>
        )}
      </For>
    </div>
  );
}
