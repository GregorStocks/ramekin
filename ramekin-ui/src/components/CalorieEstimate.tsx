import { createResource, For, Show } from "solid-js";
import type { Ingredient } from "ramekin-client";
import { useAuth } from "../context/AuthContext";

export default function CalorieEstimate(props: {
  ingredients: Ingredient[];
  servings?: string | null;
  scale: number;
}) {
  const { getRecipesApi } = useAuth();
  const [estimate] = createResource(
    () => ({
      ingredients: props.ingredients,
      servings: props.servings,
      scale: props.scale,
    }),
    (request) =>
      getRecipesApi().estimateCalories({ estimateCaloriesRequest: request }),
  );

  return (
    <section class="recipe-section" aria-label="Estimated calories">
      <h3>Estimated calories</h3>
      <Show when={estimate.loading}>
        <p role="status">Calculating calories…</p>
      </Show>
      <Show when={estimate.error}>
        <p role="alert">Could not estimate calories. Please reload to retry.</p>
      </Show>
      <Show when={!estimate.loading && !estimate.error && estimate()}>
        {(result) => (
          <>
            <p>{result().summary}</p>
            <Show when={result().perServingSummary}>
              <p>{result().perServingSummary}</p>
            </Show>
            <Show when={result().unknownIngredients.length > 0}>
              <ul>
                <For each={result().unknownIngredients}>
                  {(ingredient) => (
                    <li>
                      {ingredient.item}: {ingredient.reason}
                    </li>
                  )}
                </For>
              </ul>
            </Show>
            <p>
              Based on USDA reference foods and the listed ingredient amounts.
            </p>
          </>
        )}
      </Show>
    </section>
  );
}
