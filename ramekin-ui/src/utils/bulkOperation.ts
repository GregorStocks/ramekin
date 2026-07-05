export interface BulkOperationError {
  id: string;
  message: string;
}

export interface BulkOperationResult<TResult> {
  total: number;
  succeeded: number;
  results: TResult[];
  errors: BulkOperationError[];
}

interface RunBulkOperationOptions<TResult> {
  ids: string[];
  action: (id: string) => Promise<TResult>;
  onProgress: (done: number, total: number) => void;
  formatError: (error: unknown) => Promise<string>;
}

export async function runBulkOperation<TResult>({
  ids,
  action,
  onProgress,
  formatError,
}: RunBulkOperationOptions<TResult>): Promise<BulkOperationResult<TResult>> {
  const results: TResult[] = [];
  const errors: BulkOperationError[] = [];
  let done = 0;

  for (const id of ids) {
    try {
      results.push(await action(id));
    } catch (error) {
      errors.push({
        id,
        message: await formatError(error),
      });
    }
    done += 1;
    onProgress(done, ids.length);
  }

  return {
    total: ids.length,
    succeeded: ids.length - errors.length,
    results,
    errors,
  };
}

export function summarizeBulkErrors(errors: BulkOperationError[]): string {
  return `${errors
    .slice(0, 3)
    .map((error) => `${error.id.slice(0, 8)}: ${error.message}`)
    .join("; ")}${errors.length > 3 ? "…" : ""}`;
}
