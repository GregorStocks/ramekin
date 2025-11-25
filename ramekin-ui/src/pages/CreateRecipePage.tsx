import { createSignal, Show, Index } from "solid-js";
import { createStore } from "solid-js/store";
import { useNavigate, A } from "@solidjs/router";
import { useAuth } from "../context/AuthContext";
import type { Ingredient } from "ramekin-client";

export default function CreateRecipePage() {
  const navigate = useNavigate();
  const { getRecipesApi } = useAuth();

  const [title, setTitle] = createSignal("");
  const [description, setDescription] = createSignal("");
  const [instructions, setInstructions] = createSignal("");
  const [sourceUrl, setSourceUrl] = createSignal("");
  const [sourceName, setSourceName] = createSignal("");
  const [tagsInput, setTagsInput] = createSignal("");
  const [ingredients, setIngredients] = createStore<Ingredient[]>([
    { item: "", amount: "", unit: "" },
  ]);

  const [saving, setSaving] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  const addIngredient = () => {
    setIngredients(ingredients.length, { item: "", amount: "", unit: "" });
  };

  const removeIngredient = (index: number) => {
    setIngredients((ings) => ings.filter((_, i) => i !== index));
  };

  const updateIngredient = (
    index: number,
    field: keyof Ingredient,
    value: string,
  ) => {
    setIngredients(index, field, value);
  };

  const handleSubmit = async (e: Event) => {
    e.preventDefault();
    setError(null);
    setSaving(true);

    try {
      const validIngredients = ingredients.filter(
        (ing) => ing.item.trim() !== "",
      );
      const tags = tagsInput()
        .split(",")
        .map((t) => t.trim())
        .filter((t) => t !== "");

      const response = await getRecipesApi().createRecipe({
        createRecipeRequest: {
          title: title(),
          description: description() || undefined,
          instructions: instructions(),
          ingredients: validIngredients,
          sourceUrl: sourceUrl() || undefined,
          sourceName: sourceName() || undefined,
          tags: tags.length > 0 ? tags : undefined,
        },
      });

      navigate(`/recipes/${response.id}`);
    } catch (err) {
      if (err instanceof Response) {
        const body = await err.json();
        setError(body.error || "Failed to create recipe");
      } else {
        setError("Failed to create recipe");
      }
    } finally {
      setSaving(false);
    }
  };

  return (
    <div class="create-recipe-page">
      <h2>Create New Recipe</h2>

      <form onSubmit={handleSubmit}>
        <div class="form-group">
          <label for="title">Title *</label>
          <input
            id="title"
            type="text"
            value={title()}
            onInput={(e) => setTitle(e.currentTarget.value)}
            required
          />
        </div>

        <div class="form-group">
          <label for="description">Description</label>
          <textarea
            id="description"
            value={description()}
            onInput={(e) => setDescription(e.currentTarget.value)}
            rows={2}
          />
        </div>

        <div class="form-section">
          <div class="section-header">
            <label>Ingredients</label>
            <button type="button" class="btn btn-small" onClick={addIngredient}>
              + Add
            </button>
          </div>
          <Index each={ingredients}>
            {(ing, index) => (
              <div class="ingredient-row">
                <input
                  type="text"
                  placeholder="Amount"
                  value={ing().amount || ""}
                  onInput={(e) =>
                    updateIngredient(index, "amount", e.currentTarget.value)
                  }
                  class="input-amount"
                />
                <input
                  type="text"
                  placeholder="Unit"
                  value={ing().unit || ""}
                  onInput={(e) =>
                    updateIngredient(index, "unit", e.currentTarget.value)
                  }
                  class="input-unit"
                />
                <input
                  type="text"
                  placeholder="Ingredient *"
                  value={ing().item}
                  onInput={(e) =>
                    updateIngredient(index, "item", e.currentTarget.value)
                  }
                  class="input-item"
                />
                <input
                  type="text"
                  placeholder="Note"
                  value={ing().note || ""}
                  onInput={(e) =>
                    updateIngredient(index, "note", e.currentTarget.value)
                  }
                  class="input-note"
                />
                <button
                  type="button"
                  class="btn btn-small btn-danger"
                  onClick={() => removeIngredient(index)}
                >
                  &times;
                </button>
              </div>
            )}
          </Index>
        </div>

        <div class="form-group">
          <label for="instructions">Instructions *</label>
          <textarea
            id="instructions"
            value={instructions()}
            onInput={(e) => setInstructions(e.currentTarget.value)}
            rows={8}
            required
          />
        </div>

        <div class="form-row">
          <div class="form-group">
            <label for="sourceUrl">Source URL</label>
            <input
              id="sourceUrl"
              type="url"
              value={sourceUrl()}
              onInput={(e) => setSourceUrl(e.currentTarget.value)}
              placeholder="https://..."
            />
          </div>
          <div class="form-group">
            <label for="sourceName">Source Name</label>
            <input
              id="sourceName"
              type="text"
              value={sourceName()}
              onInput={(e) => setSourceName(e.currentTarget.value)}
              placeholder="e.g., Grandma's cookbook"
            />
          </div>
        </div>

        <div class="form-group">
          <label for="tags">Tags (comma-separated)</label>
          <input
            id="tags"
            type="text"
            value={tagsInput()}
            onInput={(e) => setTagsInput(e.currentTarget.value)}
            placeholder="e.g., dinner, easy, vegetarian"
          />
        </div>

        <Show when={error()}>
          <div class="error">{error()}</div>
        </Show>

        <div class="form-actions">
          <A href="/" class="btn">
            Cancel
          </A>
          <button type="submit" class="btn btn-primary" disabled={saving()}>
            {saving() ? "Creating..." : "Create Recipe"}
          </button>
        </div>
      </form>
    </div>
  );
}
