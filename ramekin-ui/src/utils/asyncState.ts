import { createResource, createSignal, type Accessor } from "solid-js";
import { extractApiError } from "./recipeFormHelpers";

export interface AsyncAction<TArgs extends unknown[], TResult> {
  run: (...args: TArgs) => Promise<TResult | undefined>;
  loading: Accessor<boolean>;
  error: Accessor<string | null>;
  clearError: () => void;
}

interface AsyncActionOptions {
  onError?: (message: string, err: unknown) => void | Promise<void>;
}

export function createAsyncAction<TArgs extends unknown[], TResult>(
  action: (...args: TArgs) => Promise<TResult>,
  fallbackMessage: string,
  options: AsyncActionOptions = {},
): AsyncAction<TArgs, TResult> {
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  return {
    loading,
    error,
    clearError: () => setError(null),
    run: async (...args) => {
      setLoading(true);
      setError(null);
      try {
        return await action(...args);
      } catch (err) {
        const message = await extractApiError(err, fallbackMessage);
        setError(message);
        await options.onError?.(message, err);
        return undefined;
      } finally {
        setLoading(false);
      }
    },
  };
}

export interface ApiResource<T> {
  data: Accessor<T | undefined>;
  latest: Accessor<T | undefined>;
  loading: Accessor<boolean>;
  error: Accessor<string | null>;
  refetch: () => Promise<T | undefined>;
  mutate: (value: T | undefined) => T | undefined;
}

type ApiResourceValue<T> =
  | { data: T; error: null }
  | { data?: undefined; error: string };

export function createApiResource<T>(
  fetcher: () => Promise<T>,
  fallbackMessage: string,
): ApiResource<T> {
  const [resource, { refetch, mutate }] = createResource<ApiResourceValue<T>>(
    async () => {
      try {
        return { data: await fetcher(), error: null };
      } catch (err) {
        return { error: await extractApiError(err, fallbackMessage) };
      }
    },
  );

  return {
    data: () => resource()?.data,
    latest: () => resource.latest?.data,
    loading: () => resource.loading,
    error: () => resource()?.error ?? null,
    refetch: async () => (await refetch())?.data,
    mutate: (value) => {
      mutate(value === undefined ? undefined : { data: value, error: null });
      return value;
    },
  };
}
