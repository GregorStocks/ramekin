import { createMemo, createSignal, Show } from "solid-js";
import jsPDF from "jspdf";
import Modal from "./Modal";
import type { RecipeSummary } from "ramekin-client";

interface Props {
  isOpen: () => boolean;
  onClose: () => void;
  recipes: () => RecipeSummary[];
  token: () => string | null;
}

type ImageData = {
  dataUrl: string;
  format: "JPEG" | "PNG";
  pxW: number;
  pxH: number;
};

async function blobToDataUrl(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(blob);
  });
}

async function getImagePixelSize(
  dataUrl: string,
): Promise<{ w: number; h: number }> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve({ w: img.naturalWidth, h: img.naturalHeight });
    img.onerror = () => reject(new Error("image decode failed"));
    img.src = dataUrl;
  });
}

async function fetchThumbnail(
  photoId: string,
  token: string,
): Promise<ImageData> {
  const maxAttempts = 4;
  let lastErr: unknown;
  for (let attempt = 1; attempt <= maxAttempts; attempt++) {
    try {
      const resp = await fetch(`/api/photos/${photoId}/thumbnail?size=1200`, {
        headers: { Authorization: `Bearer ${token}` },
      });
      if (!resp.ok) {
        throw new Error(
          `Failed to fetch thumbnail ${photoId}: ${resp.status} ${resp.statusText}`,
        );
      }
      const blob = await resp.blob();
      const dataUrl = await blobToDataUrl(blob);
      const format: "JPEG" | "PNG" = blob.type.includes("png") ? "PNG" : "JPEG";
      const { w, h } = await getImagePixelSize(dataUrl);
      return { dataUrl, format, pxW: w, pxH: h };
    } catch (e) {
      lastErr = e;
      if (attempt === maxAttempts) break;
      const backoffMs = 250 * 2 ** (attempt - 1);
      await new Promise((r) => setTimeout(r, backoffMs));
    }
  }
  throw lastErr instanceof Error
    ? lastErr
    : new Error(`Failed to fetch thumbnail ${photoId}`);
}

export default function PdfExportModal(props: Props) {
  const [cardW, setCardW] = createSignal(3.5);
  const [cardH, setCardH] = createSignal(2.5);
  const [marginIn, setMarginIn] = createSignal(0.75);
  const [gutterIn, setGutterIn] = createSignal(0.2);
  const [borderIn, setBorderIn] = createSignal(0);
  const [orientation, setOrientation] = createSignal<"portrait" | "landscape">(
    "portrait",
  );
  const [generating, setGenerating] = createSignal(false);
  const [progress, setProgress] = createSignal<{ done: number; total: number }>(
    { done: 0, total: 0 },
  );
  const [error, setError] = createSignal<string | null>(null);

  const pageSize = createMemo(() =>
    orientation() === "portrait"
      ? { pageW: 8.5, pageH: 11 }
      : { pageW: 11, pageH: 8.5 },
  );

  const layout = createMemo(() => {
    const { pageW, pageH } = pageSize();
    const m = marginIn();
    const g = gutterIn();
    const w = cardW();
    const h = cardH();
    const b = borderIn();
    const contentW = pageW - 2 * m;
    const contentH = pageH - 2 * m;
    const invalid = (reason: string) => ({
      cols: 0,
      rows: 0,
      perPage: 0,
      contentW,
      contentH,
      error: reason,
    });
    if (m < 0) return invalid("Margin must be non-negative.");
    if (g < 0) return invalid("Gutter must be non-negative.");
    if (w <= 0 || h <= 0) return invalid("Card size must be positive.");
    if (contentW <= 0 || contentH <= 0) {
      return invalid("Margins leave no room on the page.");
    }
    if (b < 0) return invalid("Border width must be non-negative.");
    const footW = w + b;
    const footH = h + b;
    if (footW > contentW || footH > contentH) {
      return invalid(
        "Card with border is too large for the page at these margins.",
      );
    }
    const cols = Math.max(1, Math.floor((contentW + g) / (footW + g)));
    const rows = Math.max(1, Math.floor((contentH + g) / (footH + g)));
    return {
      cols,
      rows,
      perPage: cols * rows,
      contentW,
      contentH,
      error: null as string | null,
    };
  });

  const generatePdf = async () => {
    const chosen = props.recipes();
    if (chosen.length === 0) return;
    const l = layout();
    if (l.perPage === 0) {
      setError(l.error ?? "Invalid card layout");
      return;
    }
    const { cols, perPage } = l;

    setGenerating(true);
    setError(null);
    setProgress({ done: 0, total: chosen.length });

    try {
      const tok = props.token() ?? "";
      const doc = new jsPDF({
        unit: "in",
        format: "letter",
        orientation: orientation(),
      });

      const { pageW, pageH } = pageSize();
      const g = gutterIn();
      const w = cardW();
      const h = cardH();
      const border = borderIn();
      const rows = l.rows;

      const footW = w + border;
      const footH = h + border;
      const gridW = cols * footW + (cols - 1) * g;
      const gridH = rows * footH + (rows - 1) * g;
      const startX = (pageW - gridW) / 2;
      const startY = (pageH - gridH) / 2;

      for (let i = 0; i < chosen.length; i++) {
        const recipe = chosen[i];
        const slot = i % perPage;
        if (i > 0 && slot === 0) doc.addPage();

        const col = slot % cols;
        const row = Math.floor(slot / cols);
        const x = startX + border / 2 + col * (footW + g);
        const y = startY + border / 2 + row * (footH + g);

        if (border > 0) {
          doc.setLineWidth(border);
          doc.setDrawColor(0, 0, 0);
          const bHalf = border / 2;
          doc.rect(x - bHalf, y - bHalf, w + border, h + border, "S");
        }

        const pad = 0.05;
        const innerX = x + pad;
        const innerY = y + pad;
        const innerW = w - 2 * pad;
        const innerH = h - 2 * pad;

        const titleAreaH = Math.min(0.45, Math.max(0.25, innerH * 0.22));
        const imgAreaH = innerH - titleAreaH;

        const img: ImageData | null = recipe.thumbnailPhotoId
          ? await fetchThumbnail(recipe.thumbnailPhotoId, tok)
          : null;

        if (img && imgAreaH > 0) {
          const ratio = img.pxW / img.pxH;
          let drawW = innerW;
          let drawH = drawW / ratio;
          if (drawH > imgAreaH) {
            drawH = imgAreaH;
            drawW = drawH * ratio;
          }
          const drawX = innerX + (innerW - drawW) / 2;
          const drawY = innerY + (imgAreaH - drawH) / 2;
          doc.addImage(img.dataUrl, img.format, drawX, drawY, drawW, drawH);
        } else {
          doc.setDrawColor(200);
          doc.setLineWidth(0.01);
          doc.rect(innerX, innerY, innerW, imgAreaH, "S");
          doc.setFontSize(24);
          doc.setTextColor(180);
          doc.text("\uD83C\uDF7D", innerX + innerW / 2, innerY + imgAreaH / 2, {
            align: "center",
            baseline: "middle",
          });
          doc.setTextColor(0);
        }

        const maxLines = 2;
        let fontPt = Math.min(14, Math.max(8, titleAreaH * 36));
        let lines: string[];
        while (true) {
          doc.setFontSize(fontPt);
          lines = doc.splitTextToSize(recipe.title, innerW) as string[];
          if (lines.length <= maxLines || fontPt <= 5) break;
          fontPt -= 0.5;
        }
        lines = lines.slice(0, maxLines);
        const titleCenterY = innerY + imgAreaH + titleAreaH / 2;
        doc.text(lines, innerX + innerW / 2, titleCenterY, {
          align: "center",
          baseline: "middle",
        });

        setProgress({ done: i + 1, total: chosen.length });
      }

      doc.save("recipe-cards.pdf");
      props.onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to generate PDF");
    } finally {
      setGenerating(false);
    }
  };

  return (
    <Modal
      isOpen={props.isOpen}
      onClose={props.onClose}
      title={`Export ${props.recipes().length} recipes to PDF`}
      actions={
        <>
          <button
            type="button"
            class="btn btn-small"
            onClick={props.onClose}
            disabled={generating()}
          >
            Cancel
          </button>
          <button
            type="button"
            class="btn btn-small btn-primary"
            disabled={
              generating() ||
              props.recipes().length === 0 ||
              layout().perPage === 0
            }
            onClick={generatePdf}
          >
            <Show when={generating()} fallback={<>Generate PDF</>}>
              Generating {progress().done}/{progress().total}…
            </Show>
          </button>
        </>
      }
    >
      <div class="recipe-cards-config">
        <div class="form-group">
          <label>Page orientation</label>
          <select
            value={orientation()}
            onChange={(e) =>
              setOrientation(e.currentTarget.value as "portrait" | "landscape")
            }
          >
            <option value="portrait">Portrait</option>
            <option value="landscape">Landscape</option>
          </select>
        </div>
        <div class="form-group">
          <label>Card width (in)</label>
          <input
            type="number"
            step="0.1"
            min="0.5"
            value={cardW()}
            onInput={(e) => setCardW(parseFloat(e.currentTarget.value) || 0)}
          />
        </div>
        <div class="form-group">
          <label>Card height (in)</label>
          <input
            type="number"
            step="0.1"
            min="0.5"
            value={cardH()}
            onInput={(e) => setCardH(parseFloat(e.currentTarget.value) || 0)}
          />
        </div>
        <div class="form-group">
          <label>Page margin (in)</label>
          <input
            type="number"
            step="0.05"
            min="0"
            value={marginIn()}
            onInput={(e) => setMarginIn(parseFloat(e.currentTarget.value) || 0)}
          />
        </div>
        <div class="form-group">
          <label>Gutter between cards (in)</label>
          <input
            type="number"
            step="0.05"
            min="0"
            value={gutterIn()}
            onInput={(e) => setGutterIn(parseFloat(e.currentTarget.value) || 0)}
          />
        </div>
        <div class="form-group">
          <label>Border width (in, 0 = invisible)</label>
          <input
            type="number"
            step="0.005"
            min="0"
            value={borderIn()}
            onInput={(e) => setBorderIn(parseFloat(e.currentTarget.value) || 0)}
          />
        </div>

        <p class="recipe-cards-layout-info">
          <Show when={layout().perPage > 0} fallback={<>{layout().error}</>}>
            {layout().cols}×{layout().rows} grid = {layout().perPage} cards per
            page
          </Show>
        </p>

        <Show when={error()}>
          <p class="error">{error()}</p>
        </Show>
      </div>
    </Modal>
  );
}
