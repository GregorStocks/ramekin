import { createSignal, For, Show, onMount } from "solid-js";
import { useAuth } from "../context/AuthContext";

interface TagInputProps {
  tags: () => string[];
  onTagsChange: (tags: string[]) => void;
  placeholder?: string;
  id?: string;
}

export default function TagInput(props: TagInputProps) {
  const { getTagsApi } = useAuth();

  let inputRef: HTMLInputElement | undefined;

  const [inputValue, setInputValue] = createSignal("");
  const [availableTags, setAvailableTags] = createSignal<string[]>([]);

  onMount(async () => {
    try {
      const response = await getTagsApi().listAllTags();
      setAvailableTags(response.tags.map((t) => t.name));
    } catch {
      // Ignore errors loading tags
    }
  });

  // Unselected tags, optionally filtered by input
  const unselectedTags = () => {
    const input = inputValue().toLowerCase().trim();
    const selected = props.tags();

    let tags = availableTags().filter((tag) => !selected.includes(tag));

    if (input) {
      // Filter and sort: prefix matches first, then substring matches
      const prefixMatches: string[] = [];
      const substringMatches: string[] = [];

      for (const tag of tags) {
        const lowerTag = tag.toLowerCase();
        if (lowerTag.startsWith(input)) {
          prefixMatches.push(tag);
        } else if (lowerTag.includes(input)) {
          substringMatches.push(tag);
        }
      }

      tags = [...prefixMatches, ...substringMatches];
    }

    return tags;
  };

  const showCreateOption = () => {
    const input = inputValue().trim();
    if (!input) return false;
    // Show create option if input doesn't match any existing tag exactly
    return !availableTags().some(
      (t) => t.toLowerCase() === input.toLowerCase(),
    );
  };

  const addTag = (tag: string) => {
    const normalized = tag.trim();
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
        if (input) {
          addTag(input);
        }
        break;

      case "Backspace":
        if (!inputValue() && props.tags().length > 0) {
          removeTag(props.tags()[props.tags().length - 1]);
        }
        break;
    }
  };

  const focusInput = () => inputRef?.focus();

  return (
    <div class="tag-input-container">
      {/* Selected tags */}
      <Show when={props.tags().length > 0}>
        <div class="tag-selected-list">
          <For each={props.tags()}>
            {(tag) => (
              <span class="tag-chip tag-chip-selected">
                {tag}
                <button
                  type="button"
                  class="tag-chip-remove"
                  onClick={() => removeTag(tag)}
                  aria-label={`Remove ${tag}`}
                >
                  &times;
                </button>
              </span>
            )}
          </For>
        </div>
      </Show>

      {/* Input for creating new tags */}
      <div class="tag-input-wrapper" onClick={focusInput}>
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
            onMouseDown={() => addTag(inputValue().trim())}
          >
            + Create "{inputValue().trim()}"
          </button>
        </Show>
      </div>

      {/* Available tags grid */}
      <Show when={unselectedTags().length > 0}>
        <div class="tag-available-list">
          <For each={unselectedTags()}>
            {(tag) => (
              <button
                type="button"
                class="tag-chip tag-chip-available"
                onClick={() => addTag(tag)}
              >
                {tag}
              </button>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
}
