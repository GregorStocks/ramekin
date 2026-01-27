import { createSignal, createEffect, For, Show } from "solid-js";
import { useNavigate } from "@solidjs/router";
import { useAuth } from "../context/AuthContext";
import Modal from "../components/Modal";
import { extractApiError } from "../utils/recipeFormHelpers";
import type { TagItem } from "ramekin-client";

export default function TagsPage() {
  const navigate = useNavigate();
  const { getTagsApi, refreshTags } = useAuth();

  const [tags, setTags] = createSignal<TagItem[]>([]);
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal<string | null>(null);

  // Edit state
  const [editingId, setEditingId] = createSignal<string | null>(null);
  const [editName, setEditName] = createSignal("");
  const [editError, setEditError] = createSignal<string | null>(null);
  const [saving, setSaving] = createSignal(false);

  // Delete confirmation state
  const [deleteTag, setDeleteTag] = createSignal<TagItem | null>(null);
  const [deleting, setDeleting] = createSignal(false);

  const loadTags = async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await getTagsApi().listAllTags();
      setTags(response.tags);
    } catch (err) {
      const message = await extractApiError(err, "Failed to load tags");
      setError(message);
    } finally {
      setLoading(false);
    }
  };

  createEffect(() => {
    loadTags();
  });

  const startEditing = (tag: TagItem) => {
    setEditingId(tag.id);
    setEditName(tag.name);
    setEditError(null);
  };

  const cancelEditing = () => {
    setEditingId(null);
    setEditName("");
    setEditError(null);
  };

  const handleRename = async (tagId: string) => {
    const newName = editName().trim();
    if (!newName) {
      setEditError("Tag name cannot be empty");
      return;
    }

    setSaving(true);
    setEditError(null);
    try {
      await getTagsApi().renameTag({
        id: tagId,
        renameTagRequest: { name: newName },
      });
      await refreshTags();
      await loadTags();
      cancelEditing();
    } catch (err) {
      const message = await extractApiError(err, "Failed to rename tag");
      setEditError(message);
    } finally {
      setSaving(false);
    }
  };

  const confirmDelete = (tag: TagItem) => {
    setDeleteTag(tag);
  };

  const handleDelete = async () => {
    const tag = deleteTag();
    if (!tag) return;

    setDeleting(true);
    try {
      await getTagsApi().deleteTag({ id: tag.id });
      await refreshTags();
      await loadTags();
      setDeleteTag(null);
    } catch (err) {
      const message = await extractApiError(err, "Failed to delete tag");
      setError(message);
      setDeleteTag(null);
    } finally {
      setDeleting(false);
    }
  };

  const navigateToFiltered = (tagName: string) => {
    navigate(`/?q=${encodeURIComponent(`tag:${tagName}`)}`);
  };

  const handleKeyDown = (e: KeyboardEvent, tagId: string) => {
    if (e.key === "Enter") {
      e.preventDefault();
      handleRename(tagId);
    } else if (e.key === "Escape") {
      cancelEditing();
    }
  };

  return (
    <div class="tags-page">
      <div class="page-header">
        <h2>Manage Tags</h2>
      </div>

      <Show when={error()}>
        <div class="error-message">{error()}</div>
      </Show>

      <Show when={loading()}>
        <p class="loading-text">Loading tags...</p>
      </Show>

      <Show when={!loading() && tags().length === 0}>
        <p class="empty-state">
          No tags yet. Tags are created when you add them to recipes.
        </p>
      </Show>

      <Show when={!loading() && tags().length > 0}>
        <div class="tags-list">
          <For each={tags()}>
            {(tag) => (
              <div class="tag-row">
                <Show
                  when={editingId() === tag.id}
                  fallback={
                    <>
                      <span
                        class="tag-name"
                        onClick={() => navigateToFiltered(tag.name)}
                        title="Click to view recipes with this tag"
                      >
                        {tag.name}
                      </span>
                      <span class="tag-count">
                        {tag.recipeCount}{" "}
                        {tag.recipeCount === 1 ? "recipe" : "recipes"}
                      </span>
                      <div class="tag-actions">
                        <button
                          class="btn btn-small"
                          onClick={() => startEditing(tag)}
                        >
                          Rename
                        </button>
                        <button
                          class="btn btn-small btn-danger"
                          onClick={() => confirmDelete(tag)}
                        >
                          Delete
                        </button>
                      </div>
                    </>
                  }
                >
                  <input
                    type="text"
                    class="tag-edit-input"
                    value={editName()}
                    onInput={(e) => setEditName(e.currentTarget.value)}
                    onKeyDown={(e) => handleKeyDown(e, tag.id)}
                    autofocus
                  />
                  <Show when={editError()}>
                    <span class="edit-error">{editError()}</span>
                  </Show>
                  <div class="tag-actions">
                    <button
                      class="btn btn-small btn-primary"
                      onClick={() => handleRename(tag.id)}
                      disabled={saving()}
                    >
                      {saving() ? "Saving..." : "Save"}
                    </button>
                    <button
                      class="btn btn-small"
                      onClick={cancelEditing}
                      disabled={saving()}
                    >
                      Cancel
                    </button>
                  </div>
                </Show>
              </div>
            )}
          </For>
        </div>
      </Show>

      <Modal
        isOpen={() => deleteTag() !== null}
        onClose={() => setDeleteTag(null)}
        title="Delete Tag"
        actions={
          <>
            <button
              class="btn"
              onClick={() => setDeleteTag(null)}
              disabled={deleting()}
            >
              Cancel
            </button>
            <button
              class="btn btn-danger"
              onClick={handleDelete}
              disabled={deleting()}
            >
              {deleting() ? "Deleting..." : "Delete"}
            </button>
          </>
        }
      >
        <p>Are you sure you want to delete the tag "{deleteTag()?.name}"?</p>
        <Show when={deleteTag()?.recipeCount && deleteTag()!.recipeCount > 0}>
          <p class="delete-warning">
            This will remove the tag from {deleteTag()?.recipeCount}{" "}
            {deleteTag()?.recipeCount === 1 ? "recipe" : "recipes"}.
          </p>
        </Show>
      </Modal>
    </div>
  );
}
