import {
  batch,
  createEffect,
  createResource,
  createSignal,
  For,
  onCleanup,
  Show,
} from "solid-js";
import type { Ingredient } from "ramekin-client";
import { useAuth } from "../context/AuthContext";
import { caloriePollDelaySeconds } from "../utils/caloriePolling";

export default function CalorieEstimate(props: {
  ingredients: Ingredient[];
  servings?: string | null;
  scale: number;
}) {
  const { getRecipesApi } = useAuth();
  const request = () => ({
    ingredients: props.ingredients,
    servings: props.servings,
    scale: props.scale,
  });
  const fetchEstimate = (estimateCaloriesRequest: ReturnType<typeof request>) =>
    getRecipesApi().estimateCalories({ estimateCaloriesRequest });
  // Polls made for the current estimate, which sets the next poll's delay.
  const [polls, setPolls] = createSignal(0);
  const [estimate, { mutate }] = createResource(request, (r) => {
    setPolls(0);
    return fetchEstimate(r);
  });
  // Names still being recognized in the background: ask again quietly (no
  // loading state), backing off, until the estimate includes them or the
  // schedule runs out.
  createEffect(() => {
    const delay = caloriePollDelaySeconds(polls());
    if (estimate.loading || estimate.error || !estimate()?.resolving) return;
    if (delay === null) return;
    const polled = request();
    const current = () =>
      !estimate.loading &&
      props.ingredients === polled.ingredients &&
      props.servings === polled.servings &&
      props.scale === polled.scale;
    const timer = setTimeout(async () => {
      let result: Awaited<ReturnType<typeof fetchEstimate>> | undefined;
      try {
        result = await fetchEstimate(polled);
      } catch {
        // Keep showing the last good estimate; the next poll may recover.
      }
      if (!current()) return;
      batch(() => {
        // Most polls return the same estimate; re-rendering it every poll is
        // wasted work.
        if (result && JSON.stringify(result) !== JSON.stringify(estimate())) {
          mutate(result);
        }
        setPolls((n) => n + 1);
      });
    }, delay * 1000);
    onCleanup(() => clearTimeout(timer));
  });

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
                  amounts, with model estimates where noted.
                </p>
              </details>
            </Show>
          </>
        )}
      </Show>
    </section>
  );
}
