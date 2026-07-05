import { createSignal, For, Show } from "solid-js";
import { useNavigate } from "@solidjs/router";
import { useAuth } from "../context/AuthContext";
import Modal from "../components/Modal";
import { usePageTitle } from "../utils/pageTitle";
import type { TagItem } from "ramekin-client";
import { groupTags, parseTag } from "../utils/tagHierarchy";
import { createApiResource, createAsyncAction } from "../utils/asyncState";

export default function TagsPage() {
  usePageTitle(() => "Tags");
  const navigate = useNavigate();
  const { getTagsApi, refreshTags } = useAuth();

  const tagsResource = createApiResource(
    async () => getTagsApi().listAllTags(),
    "Failed to load tags",
  );
  const tags = () => tagsResource.data()?.tags ?? [];
  const groupedTags = () => {
    const byName = new Map(tags().map((t) => [t.name, t] as const));
    return groupTags(tags().map((t) => t.name)).map((group) => ({
      namespace: group.namespace,
      items: group.tags.map((name) => byName.get(name)!).filter(Boolean),
    }));
  };
  const loading = tagsResource.loading;

  // Edit state
  const [editingId, setEditingId] = createSignal<string | null>(null);
  const [editName, setEditName] = createSignal("");
  const [editValidationError, setEditValidationError] = createSignal<
    string | null
  >(null);

  // Delete confirmation state
  const [deleteTag, setDeleteTag] = createSignal<TagItem | null>(null);

  // Bulk edit state
  const [bulkEditing, setBulkEditing] = createSignal(false);
  const [bulkNames, setBulkNames] = createSignal<Record<string, string>>({});
  const [bulkErrors, setBulkErrors] = createSignal<Record<string, string>>({});

  const renameAction = createAsyncAction(async (tagId: string) => {
    const newName = editName().trim();
    await getTagsApi().renameTag({
      id: tagId,
      renameTagRequest: { name: newName },
    });
    await refreshTags();
    await tagsResource.refetch();
    cancelEditing();
  }, "Failed to rename tag");

  const bulkRenameAction = createAsyncAction(async () => {
    for (const tag of pendingBulkChanges()) {
      await getTagsApi().renameTag({
        id: tag.id,
        renameTagRequest: { name: temporaryBulkName(tag) },
      });
    }
    for (const tag of pendingBulkChanges()) {
      await getTagsApi().renameTag({
        id: tag.id,
        renameTagRequest: { name: normalizedBulkName(tag) },
      });
    }
    await refreshTags();
    await tagsResource.refetch();
    cancelBulkEditing();
  }, "Failed to rename tags");

  const deleteAction = createAsyncAction(
    async (tag: TagItem) => {
      await getTagsApi().deleteTag({ id: tag.id });
      await refreshTags();
      await tagsResource.refetch();
      setDeleteTag(null);
    },
    "Failed to delete tag",
    {
      onError: () => {
        setDeleteTag(null);
      },
    },
  );

  const saving = renameAction.loading;
  const deleting = deleteAction.loading;
  const bulkSaving = bulkRenameAction.loading;
  const editError = () => editValidationError() ?? renameAction.error();
  const error = () =>
    bulkRenameAction.error() ?? deleteAction.error() ?? tagsResource.error();

  const clearPageErrors = () => {
    bulkRenameAction.clearError();
    deleteAction.clearError();
  };

  const startEditing = (tag: TagItem) => {
    setEditingId(tag.id);
    setEditName(tag.name);
    setEditValidationError(null);
    renameAction.clearError();
  };

  const cancelEditing = () => {
    setEditingId(null);
    setEditName("");
    setEditValidationError(null);
    renameAction.clearError();
  };

  const startBulkEditing = () => {
    setEditingId(null);
    setEditName("");
    setEditValidationError(null);
    renameAction.clearError();
    clearPageErrors();
    setBulkErrors({});
    setBulkNames(
      Object.fromEntries(tags().map((tag) => [tag.id, tag.name] as const)),
    );
    setBulkEditing(true);
  };

  const cancelBulkEditing = () => {
    setBulkEditing(false);
    setBulkNames({});
    setBulkErrors({});
    bulkRenameAction.clearError();
  };

  const updateBulkName = (tagId: string, value: string) => {
    setBulkNames((current) => ({ ...current, [tagId]: value }));
    setBulkErrors((current) => {
      if (!current[tagId]) return current;
      const next = { ...current };
      delete next[tagId];
      return next;
    });
  };

  const normalizedBulkName = (tag: TagItem) =>
    (bulkNames()[tag.id] ?? tag.name).trim();

  const pendingBulkChanges = () =>
    tags().filter((tag) => normalizedBulkName(tag) !== tag.name);

  const temporaryBulkName = (tag: TagItem) => `__bulk_rename_${tag.id}`;

  const validateBulkChanges = () => {
    const nextErrors: Record<string, string> = {};
    const byNormalizedName = new Map<string, string[]>();

    for (const tag of tags()) {
      const nextName = normalizedBulkName(tag);
      if (!nextName) {
        nextErrors[tag.id] = "Tag name cannot be empty";
        continue;
      }

      const key = nextName.toLowerCase();
      const ids = byNormalizedName.get(key) ?? [];
      ids.push(tag.id);
      byNormalizedName.set(key, ids);
    }

    for (const ids of byNormalizedName.values()) {
      if (ids.length < 2) continue;
      for (const id of ids) {
        nextErrors[id] = "Two tags cannot end up with the same name";
      }
    }

    setBulkErrors(nextErrors);
    return Object.keys(nextErrors).length === 0;
  };

  const handleRename = async (tagId: string) => {
    const newName = editName().trim();
    if (!newName) {
      setEditValidationError("Tag name cannot be empty");
      return;
    }

    setEditValidationError(null);
    await renameAction.run(tagId);
  };

  const handleBulkSave = async () => {
    if (!validateBulkChanges()) return;

    const changedTags = pendingBulkChanges();
    if (changedTags.length === 0) {
      cancelBulkEditing();
      return;
    }

    await bulkRenameAction.run();
  };

  const confirmDelete = (tag: TagItem) => {
    setDeleteTag(tag);
  };

  const handleDelete = async () => {
    const tag = deleteTag();
    if (!tag) return;

    await deleteAction.run(tag);
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
        <div class="tags-page-actions">
          <Show
            when={bulkEditing()}
            fallback={
              <button class="btn btn-primary" onClick={startBulkEditing}>
                Bulk rename
              </button>
            }
          >
            <button
              class="btn btn-primary"
              onClick={handleBulkSave}
              disabled={bulkSaving()}
            >
              {bulkSaving()
                ? "Saving..."
                : `Save ${pendingBulkChanges().length} change${
                    pendingBulkChanges().length === 1 ? "" : "s"
                  }`}
            </button>
            <button
              class="btn"
              onClick={cancelBulkEditing}
              disabled={bulkSaving()}
            >
              Cancel
            </button>
          </Show>
        </div>
      </div>

      <p class="tags-hierarchy-help">
        Use <code>namespace:value</code> names to build the tag hierarchy.
        Rename existing tags into the desired namespace, such as{" "}
        <code>course:breakfast</code>, <code>cuisine:mexican</code>, or{" "}
        <code>diet:vegetarian</code>.
      </p>

      <Show when={bulkEditing()}>
        <p class="tags-bulk-help">
          Edit tag names in place, including namespace changes like{" "}
          <code>course:breakfast</code>. Only changed rows will be saved.
        </p>
      </Show>

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
          <For each={groupedTags()}>
            {(group) => (
              <section class="tags-group">
                <h3 class="tags-group-label">
                  {group.namespace ?? "Uncategorized"}
                </h3>
                <For each={group.items}>
                  {(tag) => {
                    const parsed = parseTag(tag.name);
                    return (
                      <div class="tag-row">
                        <Show
                          when={bulkEditing()}
                          fallback={
                            <Show
                              when={editingId() === tag.id}
                              fallback={
                                <>
                                  <span
                                    class="tag-name"
                                    onClick={() => navigateToFiltered(tag.name)}
                                    title="Click to view recipes with this tag"
                                  >
                                    <Show when={parsed.namespace}>
                                      <span class="tag-chip-ns">
                                        {parsed.namespace}:
                                      </span>
                                    </Show>
                                    {parsed.value}
                                  </span>
                                  <span class="tag-count">
                                    {tag.recipeCount}{" "}
                                    {tag.recipeCount === 1
                                      ? "recipe"
                                      : "recipes"}
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
                                onInput={(e) =>
                                  setEditName(e.currentTarget.value)
                                }
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
                          }
                        >
                          <div class="tag-bulk-fields">
                            <label
                              class="tag-bulk-label"
                              for={`bulk-tag-${tag.id}`}
                            >
                              Rename {tag.name}
                            </label>
                            <input
                              id={`bulk-tag-${tag.id}`}
                              type="text"
                              class="tag-edit-input"
                              value={bulkNames()[tag.id] ?? tag.name}
                              onInput={(e) =>
                                updateBulkName(tag.id, e.currentTarget.value)
                              }
                              aria-label={`Rename tag ${tag.name}`}
                            />
                            <Show when={bulkErrors()[tag.id]}>
                              <span class="edit-error">
                                {bulkErrors()[tag.id]}
                              </span>
                            </Show>
                          </div>
                          <span class="tag-count">
                            {tag.recipeCount}{" "}
                            {tag.recipeCount === 1 ? "recipe" : "recipes"}
                          </span>
                          <div class="tag-actions">
                            <button
                              class="btn btn-small btn-danger"
                              onClick={() => confirmDelete(tag)}
                              disabled={bulkSaving()}
                            >
                              Delete
                            </button>
                          </div>
                        </Show>
                      </div>
                    );
                  }}
                </For>
              </section>
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
