import { Show, Index, For, createMemo, createSignal } from "solid-js";
import { produce } from "solid-js/store";
import type { SetStoreFunction } from "solid-js/store";
import {
  DragDropProvider,
  DragDropSensors,
  DragOverlay,
  createDraggable,
  createDroppable,
  closestCenter,
} from "@thisbeyond/solid-dnd";
import type { DragEvent } from "@thisbeyond/solid-dnd";
import type { Ingredient } from "ramekin-client";
import {
  getMeasurementAmount,
  getMeasurementUnit,
} from "../utils/recipeFormHelpers";
import {
  emptyIngredientRow,
  sectionEndIndex,
  sectionRow,
} from "../utils/ingredientEditorRows";
import type { IngredientEditorRow } from "../utils/ingredientEditorRows";

type RowsSetter = SetStoreFunction<IngredientEditorRow[]>;

/** Where the dragged ingredient would land relative to a hovered row. */
type DropPosition = "before" | "after" | null;

function updateIngredient(
  setRows: RowsSetter,
  rowIndex: number,
  update: (ingredient: Ingredient) => void,
) {
  setRows(
    rowIndex,
    produce((row) => {
      if (row.kind !== "ingredient") {
        throw new Error(`Row ${rowIndex} is not an ingredient`);
      }
      update(row.ingredient);
    }),
  );
}

function setMeasurementField(
  ingredient: Ingredient,
  measurementIndex: number,
  field: "amount" | "unit",
  value: string | undefined,
) {
  ingredient.measurements[measurementIndex] = {
    ...ingredient.measurements[measurementIndex],
    [field]: value,
  };
}

function IngredientRowEditor(props: {
  row: Extract<IngredientEditorRow, { kind: "ingredient" }>;
  index: number;
  inSection: boolean;
  dropPosition: DropPosition;
  setRows: RowsSetter;
}) {
  const draggable = createDraggable(props.row.id);
  const droppable = createDroppable(props.row.id);
  const ing = () => props.row.ingredient;
  const update = (fn: (ingredient: Ingredient) => void) =>
    updateIngredient(props.setRows, props.index, fn);

  return (
    <div
      ref={(el) => {
        draggable.ref(el);
        droppable.ref(el);
      }}
      class="ingredient-entry ingredient-editor-row"
      classList={{
        "is-dragging": draggable.isActiveDraggable,
        "in-section": props.inSection,
        "drop-before": props.dropPosition === "before",
        "drop-after": props.dropPosition === "after",
      }}
    >
      <div class="ingredient-row">
        <span
          class="drag-handle"
          title="Drag to move"
          aria-label="Drag to move ingredient"
          {...draggable.dragActivators}
        >
          ⋮⋮
        </span>
        <input
          type="text"
          placeholder="Amount"
          aria-label="Amount"
          value={getMeasurementAmount(ing(), 0)}
          onInput={(e) => {
            const value = e.currentTarget.value || undefined;
            update((i) => {
              setMeasurementField(i, 0, "amount", value);
            });
          }}
          class="input-amount"
        />
        <input
          type="text"
          placeholder="Unit"
          aria-label="Unit"
          value={getMeasurementUnit(ing(), 0)}
          onInput={(e) => {
            const value = e.currentTarget.value || undefined;
            update((i) => {
              setMeasurementField(i, 0, "unit", value);
            });
          }}
          class="input-unit"
        />
        <input
          type="text"
          placeholder="Ingredient *"
          aria-label="Ingredient"
          value={ing().item}
          onInput={(e) => {
            const value = e.currentTarget.value;
            update((i) => {
              i.item = value;
            });
          }}
          class="input-item"
        />
        <input
          type="text"
          placeholder="Note"
          aria-label="Ingredient note"
          value={ing().note || ""}
          onInput={(e) => {
            const value = e.currentTarget.value || undefined;
            update((i) => {
              i.note = value;
            });
          }}
          class="input-note"
        />
        <button
          type="button"
          class="btn btn-small btn-add-alt"
          onClick={() =>
            update((i) => {
              i.measurements.push({});
            })
          }
          title="Add alternative measurement"
          aria-label="Add alternative measurement"
        >
          +
        </button>
        <button
          type="button"
          class="btn btn-small btn-remove"
          onClick={() =>
            props.setRows((rows) => rows.filter((_, i) => i !== props.index))
          }
          aria-label="Remove ingredient"
        >
          &times;
        </button>
      </div>
      <Show when={ing().measurements.length > 1}>
        <div class="alt-measurements">
          <Index each={ing().measurements.slice(1)}>
            {(_, altIndex) => {
              const mIndex = altIndex + 1;
              return (
                <div class="alt-measurement-row">
                  <span class="alt-label">Alt:</span>
                  <input
                    type="text"
                    placeholder="Amount"
                    aria-label="Amount"
                    value={getMeasurementAmount(ing(), mIndex)}
                    onInput={(e) => {
                      const value = e.currentTarget.value || undefined;
                      update((i) => {
                        setMeasurementField(i, mIndex, "amount", value);
                      });
                    }}
                    class="input-amount"
                  />
                  <input
                    type="text"
                    placeholder="Unit"
                    aria-label="Unit"
                    value={getMeasurementUnit(ing(), mIndex)}
                    onInput={(e) => {
                      const value = e.currentTarget.value || undefined;
                      update((i) => {
                        setMeasurementField(i, mIndex, "unit", value);
                      });
                    }}
                    class="input-unit"
                  />
                  <button
                    type="button"
                    class="btn btn-small btn-remove"
                    onClick={() =>
                      update((i) => {
                        i.measurements.splice(mIndex, 1);
                      })
                    }
                    title="Remove alternative measurement"
                    aria-label="Remove alternative measurement"
                  >
                    &times;
                  </button>
                </div>
              );
            }}
          </Index>
        </div>
      </Show>
    </div>
  );
}

function SectionRowEditor(props: {
  row: Extract<IngredientEditorRow, { kind: "section" }>;
  index: number;
  autofocus: boolean;
  dropPosition: DropPosition;
  setRows: RowsSetter;
}) {
  // Section headings are drop targets (dropping next to one moves an
  // ingredient into or out of that section) but are not draggable themselves.
  const droppable = createDroppable(props.row.id);

  return (
    <div
      ref={(el) => droppable.ref(el)}
      class="ingredient-editor-row ingredient-section-header-row"
      classList={{
        "drop-before": props.dropPosition === "before",
        "drop-after": props.dropPosition === "after",
      }}
    >
      <input
        ref={(el) => {
          if (props.autofocus) queueMicrotask(() => el.focus());
        }}
        type="text"
        class="section-name-input"
        value={props.row.name}
        placeholder="Section name"
        aria-label="Section name"
        onInput={(e) => {
          const value = e.currentTarget.value;
          props.setRows(
            props.index,
            produce((row) => {
              if (row.kind !== "section") {
                throw new Error(`Row ${props.index} is not a section`);
              }
              row.name = value;
            }),
          );
        }}
      />
      <button
        type="button"
        class="btn btn-small"
        onClick={() =>
          props.setRows(
            produce((rows) => {
              rows.splice(
                sectionEndIndex(rows, props.index),
                0,
                emptyIngredientRow(),
              );
            }),
          )
        }
        title="Add ingredient to this section"
        aria-label="Add ingredient to this section"
      >
        +
      </button>
      <button
        type="button"
        class="btn btn-small btn-remove"
        onClick={() =>
          props.setRows((rows) => rows.filter((_, i) => i !== props.index))
        }
        title="Delete section (its ingredients move to the section above)"
        aria-label="Delete section"
      >
        &times;
      </button>
    </div>
  );
}

/**
 * Ingredient list editor. Ingredients and section headings are rows of one
 * list, so dragging an ingredient past a heading moves it into that section.
 * Rows stay put while dragging and an insertion line marks the drop point,
 * which stays accurate even though rows have different heights.
 */
export default function IngredientEditor(props: {
  rows: IngredientEditorRow[];
  setRows: RowsSetter;
}) {
  const [dropTarget, setDropTarget] = createSignal<{
    id: string;
    position: "before" | "after";
  } | null>(null);
  const [focusRowId, setFocusRowId] = createSignal<string | null>(null);

  const rowIds = () => props.rows.map((row) => row.id);

  const sectionedRowIds = createMemo(() => {
    const ids = new Set<string>();
    let inSection = false;
    for (const row of props.rows) {
      if (row.kind === "section") inSection = row.name.trim() !== "";
      else if (inSection) ids.add(row.id);
    }
    return ids;
  });

  const dropPosition = (rowId: string): DropPosition => {
    const target = dropTarget();
    return target?.id === rowId ? target.position : null;
  };

  // Dropping on a row below the dragged one lands after it; on a row above
  // lands before it. That matches removing the row and re-inserting it at the
  // target's index.
  const onDragOver = ({ draggable, droppable }: DragEvent) => {
    if (!droppable || draggable.id === droppable.id) {
      setDropTarget(null);
      return;
    }
    const ids = rowIds();
    const from = ids.indexOf(draggable.id as string);
    const to = ids.indexOf(droppable.id as string);
    setDropTarget({
      id: droppable.id as string,
      position: from < to ? "after" : "before",
    });
  };

  const onDragEnd = ({ draggable, droppable }: DragEvent) => {
    setDropTarget(null);
    if (!droppable || draggable.id === droppable.id) return;
    const ids = rowIds();
    const from = ids.indexOf(draggable.id as string);
    const to = ids.indexOf(droppable.id as string);
    props.setRows(
      produce((rows) => {
        const [moved] = rows.splice(from, 1);
        rows.splice(to, 0, moved);
      }),
    );
  };

  const addSection = () => {
    const heading = sectionRow("");
    setFocusRowId(heading.id);
    props.setRows(
      produce((rows) => {
        rows.push(heading, emptyIngredientRow());
      }),
    );
  };

  return (
    <div class="form-section">
      <div class="section-header">
        <label>Ingredients</label>
        <div class="section-header-actions">
          <button
            type="button"
            class="btn btn-small"
            onClick={() =>
              props.setRows(
                produce((rows) => {
                  rows.push(emptyIngredientRow());
                }),
              )
            }
          >
            + Ingredient
          </button>
          <button type="button" class="btn btn-small" onClick={addSection}>
            + Section
          </button>
        </div>
      </div>

      <DragDropProvider
        onDragOver={onDragOver}
        onDragEnd={onDragEnd}
        collisionDetector={closestCenter}
      >
        <DragDropSensors />
        <div class="ingredient-editor">
          <For each={props.rows}>
            {(row, index) => (
              <Show
                when={row.kind === "section" ? row : undefined}
                fallback={
                  <IngredientRowEditor
                    row={
                      row as Extract<
                        IngredientEditorRow,
                        { kind: "ingredient" }
                      >
                    }
                    index={index()}
                    inSection={sectionedRowIds().has(row.id)}
                    dropPosition={dropPosition(row.id)}
                    setRows={props.setRows}
                  />
                }
              >
                {(section) => (
                  <SectionRowEditor
                    row={section()}
                    index={index()}
                    autofocus={focusRowId() === row.id}
                    dropPosition={dropPosition(row.id)}
                    setRows={props.setRows}
                  />
                )}
              </Show>
            )}
          </For>
        </div>
        <DragOverlay>
          {(draggable) => {
            const row = props.rows.find((r) => r.id === draggable?.id);
            return row?.kind === "ingredient" ? (
              <div class="ingredient-entry drag-overlay">
                <span class="drag-handle">⋮⋮</span>
                <span>
                  {[
                    getMeasurementAmount(row.ingredient, 0),
                    getMeasurementUnit(row.ingredient, 0),
                    row.ingredient.item,
                  ]
                    .filter(Boolean)
                    .join(" ") || "Ingredient"}
                </span>
              </div>
            ) : null;
          }}
        </DragOverlay>
      </DragDropProvider>

      <Show when={props.rows.length === 0}>
        <p class="empty-ingredients">No ingredients yet</p>
      </Show>
    </div>
  );
}
