import { createSignal, For, Show } from "solid-js";
import { useAuth } from "../context/AuthContext";
import {
  formatTag,
  groupTags,
  knownNamespaces,
  normalizeNamespace,
  parseTag,
} from "../utils/tagHierarchy";

interface TagInputProps {
  tags: () => string[];
  onTagsChange: (tags: string[]) => void;
  placeholder?: string;
  id?: string;
}

const NONE = "";
const NEW_NS = "__new__";

export default function TagInput(props: TagInputProps) {
  const { tags: availableTags } = useAuth();

  let inputRef: HTMLInputElement | undefined;

  const [inputValue, setInputValue] = createSignal("");
  const [namespace, setNamespace] = createSignal<string>(NONE);
  const [newNsValue, setNewNsValue] = createSignal("");
  const [newNsError, setNewNsError] = createSignal<string | null>(null);

  const namespaces = () => knownNamespaces(availableTags());

  const unselectedGroups = () => {
    const input = inputValue().toLowerCase().trim();
    const selected = new Set(props.tags());
    const available = availableTags().filter((t) => !selected.has(t));
    const filtered = input
      ? available.filter((t) => t.toLowerCase().includes(input))
      : available;
    return groupTags(filtered);
  };

  const effectiveTagName = (raw: string): string => {
    const trimmed = raw.trim();
    if (!trimmed) return "";
    // If the user typed a colon, honor it as an inline namespace override.
    if (trimmed.includes(":")) return trimmed;
    const ns = namespace();
    if (!ns || ns === NEW_NS) return trimmed;
    return formatTag(ns, trimmed);
  };

  const showCreateOption = () => {
    const name = effectiveTagName(inputValue());
    if (!name) return false;
    return !availableTags().some((t) => t.toLowerCase() === name.toLowerCase());
  };

  const addTag = (tagName: string) => {
    const normalized = tagName.trim();
    if (normalized && !props.tags().includes(normalized)) {
      props.onTagsChange([...props.tags(), normalized]);
    }
    setInputValue("");
  };

  const removeTag = (tagToRemove: string) => {
    props.onTagsChange(props.tags().filter((t) => t !== tagToRemove));
  };

  const handleKeyDown = (e: KeyboardEvent) => {
    const input = inputValue().trim();
    switch (e.key) {
      case "Enter":
      case ",":
        e.preventDefault();
        if (input) addTag(effectiveTagName(input));
        break;
      case "Backspace":
        if (!inputValue() && props.tags().length > 0) {
          removeTag(props.tags()[props.tags().length - 1]);
        }
        break;
    }
  };

  const acceptNewNamespace = () => {
    if (!newNsValue().trim()) {
      setNewNsError("Namespace cannot be empty");
      return;
    }
    const ns = normalizeNamespace(newNsValue());
    if (!ns) {
      setNewNsError(
        "Use lowercase letters, digits, - or _, starting with a letter",
      );
      return;
    }
    setNewNsError(null);
    setNamespace(ns);
    setNewNsValue("");
  };

  const focusInput = () => inputRef?.focus();

  return (
    <div class="tag-input-container">
      <Show when={props.tags().length > 0}>
        <div class="tag-selected-list">
          <For each={props.tags()}>
            {(tag) => {
              const parsed = parseTag(tag);
              return (
                <span class="tag-chip tag-chip-selected">
                  <Show when={parsed.namespace}>
                    <span class="tag-chip-ns">{parsed.namespace}:</span>
                  </Show>
                  {parsed.value}
                  <button
                    type="button"
                    class="tag-chip-remove"
                    onClick={() => removeTag(tag)}
                    aria-label={`Remove ${tag}`}
                  >
                    &times;
                  </button>
                </span>
              );
            }}
          </For>
        </div>
      </Show>

      <div class="tag-input-wrapper" onClick={focusInput}>
        <select
          class="tag-input-namespace"
          value={namespace()}
          onChange={(e) => {
            const val = e.currentTarget.value;
            setNamespace(val);
            if (val !== NEW_NS) setNewNsValue("");
          }}
          aria-label="Tag namespace"
        >
          <option value={NONE}>(none)</option>
          <For each={namespaces()}>
            {(ns) => <option value={ns}>{ns}</option>}
          </For>
          <option value={NEW_NS}>+ New namespace…</option>
        </select>
        <input
          ref={inputRef}
          type="text"
          class="tag-input-field"
          id={props.id}
          value={inputValue()}
          onInput={(e) => setInputValue(e.currentTarget.value)}
          onKeyDown={handleKeyDown}
          placeholder={props.placeholder ?? "Type to create new tag..."}
        />
        <Show when={showCreateOption()}>
          <button
            type="button"
            class="tag-create-btn"
            onMouseDown={() => addTag(effectiveTagName(inputValue()))}
          >
            + Create "{effectiveTagName(inputValue())}"
          </button>
        </Show>
      </div>

      <Show when={namespace() === NEW_NS}>
        <div class="tag-input-new-ns">
          <input
            type="text"
            value={newNsValue()}
            placeholder="new-namespace"
            onInput={(e) => setNewNsValue(e.currentTarget.value)}
          />
          <button type="button" onMouseDown={acceptNewNamespace}>
            Add namespace
          </button>
          <Show when={newNsError()}>
            <div class="tag-input-error">{newNsError()}</div>
          </Show>
        </div>
      </Show>

      <Show when={unselectedGroups().length > 0}>
        <div class="tag-available-list">
          <For each={unselectedGroups()}>
            {(group) => (
              <div class="tag-available-group">
                <div class="tag-available-group-label">
                  {group.namespace ?? "Uncategorized"}
                </div>
                <div class="tag-available-group-chips">
                  <For each={group.tags}>
                    {(tag) => {
                      const parsed = parseTag(tag);
                      return (
                        <button
                          type="button"
                          class="tag-chip tag-chip-available"
                          onClick={() => addTag(tag)}
                        >
                          <Show when={parsed.namespace}>
                            <span class="tag-chip-ns">{parsed.namespace}:</span>
                          </Show>
                          {parsed.value}
                        </button>
                      );
                    }}
                  </For>
                </div>
              </div>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
}
