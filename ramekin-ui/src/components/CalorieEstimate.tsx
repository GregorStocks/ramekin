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
            <p class="calorie-headline">{result().headline}</p>
            <Show when={result().secondary}>
              <p class="calorie-secondary">{result().secondary}</p>
            </Show>
            <Show when={result().notCounted.length > 0}>
              <p class="calorie-secondary">
                Not counted: {result().notCounted.join(", ")}
              </p>
            </Show>
            <Show when={result().lines.length > 0}>
              <details class="calorie-breakdown">
                <summary>How is this calculated?</summary>
                <ul>
                  <For each={result().lines}>
                    {(line) => (
                      <li>
                        <span>{line.item}</span>
                        <span class="calorie-line-text">{line.text}</span>
                      </li>
                    )}
                  </For>
                </ul>
                <p class="calorie-secondary">
                  Based on USDA reference foods and the listed ingredient
                  amounts.
                </p>
              </details>
            </Show>
          </>
        )}
      </Show>
    </section>
  );
}
