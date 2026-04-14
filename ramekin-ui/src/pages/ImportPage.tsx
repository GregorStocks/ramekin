import { createSignal, Show, For } from "solid-js";
import { useAuth } from "../context/AuthContext";
import {
  parsePaprikaArchive,
  type ParsedPaprikaRecipe,
} from "../utils/paprikaImport";
import { usePageTitle } from "../utils/pageTitle";
import { ImportExtractionMethod } from "ramekin-client";

type RecipeStatus =
  | { state: "pending" }
  | { state: "importing" }
  | { state: "done"; jobId: string }
  | { state: "error"; message: string };

interface RecipeRow {
  name: string;
  status: RecipeStatus;
}

export default function ImportPage() {
  usePageTitle(() => "Import");
  const { getPhotosApi, getImportApi } = useAuth();
  const [rows, setRows] = createSignal<RecipeRow[]>([]);
  const [busy, setBusy] = createSignal(false);
  const [fileError, setFileError] = createSignal<string | null>(null);

  const handleFile = async (event: Event) => {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;

    setFileError(null);
    setRows([]);
    setBusy(true);

    let parsed: ParsedPaprikaRecipe[];
    try {
      const buf = new Uint8Array(await file.arrayBuffer());
      parsed = parsePaprikaArchive(buf);
    } catch (err) {
      setFileError(
        err instanceof Error ? err.message : "Failed to parse archive",
      );
      setBusy(false);
      input.value = "";
      return;
    }

    if (parsed.length === 0) {
      setFileError("No recipes found in archive");
      setBusy(false);
      input.value = "";
      return;
    }

    setRows(
      parsed.map((r) => ({ name: r.name, status: { state: "pending" } })),
    );

    const photosApi = getPhotosApi();
    const importApi = getImportApi();

    for (let i = 0; i < parsed.length; i++) {
      const recipe = parsed[i];
      updateRow(i, { state: "importing" });

      try {
        const photoIds: string[] = [];
        for (const photoBytes of recipe.photos) {
          const blob = new Blob([photoBytes as BlobPart], {
            type: "image/jpeg",
          });
          const response = await photosApi.upload({
            file: new File([blob], "photo.jpg", { type: "image/jpeg" }),
          });
          photoIds.push(response.id);
        }

        const response = await importApi.importRecipe({
          importRecipeRequest: {
            rawRecipe: recipe.rawRecipe,
            photoIds,
            extractionMethod: ImportExtractionMethod.Paprika,
          },
        });
        updateRow(i, { state: "done", jobId: response.jobId });
      } catch (err) {
        updateRow(i, {
          state: "error",
          message: err instanceof Error ? err.message : String(err),
        });
      }
    }

    setBusy(false);
    input.value = "";
  };

  const updateRow = (index: number, status: RecipeStatus) => {
    setRows((prev) => {
      const next = prev.slice();
      next[index] = { ...next[index], status };
      return next;
    });
  };

  const statusText = (s: RecipeStatus) => {
    switch (s.state) {
      case "pending":
        return "Waiting...";
      case "importing":
        return "Importing...";
      case "done":
        return "Imported";
      case "error":
        return `Error: ${s.message}`;
    }
  };

  const summary = () => {
    const all = rows();
    if (all.length === 0) return null;
    const done = all.filter((r) => r.status.state === "done").length;
    const errors = all.filter((r) => r.status.state === "error").length;
    return `${done} imported, ${errors} failed, ${all.length} total`;
  };

  return (
    <div class="import-page">
      <h1>Import Paprika Recipes</h1>
      <p>
        Upload a <code>.paprikarecipes</code> file exported from Paprika. Each
        recipe is queued for import; photos are uploaded first.
      </p>
      <input
        type="file"
        accept=".paprikarecipes,application/zip"
        onChange={handleFile}
        disabled={busy()}
      />
      <Show when={fileError()}>
        <p class="error">{fileError()}</p>
      </Show>
      <Show when={rows().length > 0}>
        <p>{summary()}</p>
        <ul class="import-list">
          <For each={rows()}>
            {(row) => (
              <li>
                <strong>{row.name}</strong> — {statusText(row.status)}
              </li>
            )}
          </For>
        </ul>
      </Show>
    </div>
  );
}
