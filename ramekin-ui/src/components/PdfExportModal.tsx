import { createMemo, createSignal, Show } from "solid-js";
import jsPDF from "jspdf";
import QRCode from "qrcode";
import Modal from "./Modal";
import type { RecipeSummary } from "ramekin-client";

declare const __EXTERNAL_URL__: string;

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

type FlipDirection = "long-edge" | "short-edge";

const PHOTO_ASPECT_RATIO = 3 / 2;
const BACK_CARD_PAD_DEFAULT = 0.1;

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

async function cropImageToAspectRatio(
  img: ImageData,
  targetRatio: number,
): Promise<ImageData> {
  if (targetRatio <= 0) return img;

  const sourceRatio = img.pxW / img.pxH;
  if (Math.abs(sourceRatio - targetRatio) < 0.01) return img;

  const image = new Image();
  image.src = img.dataUrl;
  await image.decode();

  let sourceX = 0;
  let sourceY = 0;
  let sourceW = img.pxW;
  let sourceH = img.pxH;

  if (sourceRatio > targetRatio) {
    sourceW = Math.round(img.pxH * targetRatio);
    sourceX = Math.round((img.pxW - sourceW) / 2);
  } else {
    sourceH = Math.round(img.pxW / targetRatio);
    sourceY = Math.round((img.pxH - sourceH) / 2);
  }

  const canvas = document.createElement("canvas");
  canvas.width = sourceW;
  canvas.height = sourceH;

  const ctx = canvas.getContext("2d");
  if (!ctx) {
    throw new Error("Failed to create canvas context for image crop");
  }

  ctx.drawImage(
    image,
    sourceX,
    sourceY,
    sourceW,
    sourceH,
    0,
    0,
    sourceW,
    sourceH,
  );

  const mimeType = img.format === "PNG" ? "image/png" : "image/jpeg";
  return {
    dataUrl: canvas.toDataURL(mimeType, 0.92),
    format: img.format,
    pxW: sourceW,
    pxH: sourceH,
  };
}

function drawCutGuides(
  doc: jsPDF,
  pageW: number,
  pageH: number,
  xCuts: number[],
  yCuts: number[],
  gridBounds: { left: number; right: number; top: number; bottom: number },
) {
  const edgeInset = 0.08;
  const desiredGuideLen = 0.22;
  const guideGap = 0.04;

  const topGuideLen = Math.max(
    0,
    Math.min(desiredGuideLen, gridBounds.top - edgeInset - guideGap),
  );
  const bottomGuideLen = Math.max(
    0,
    Math.min(desiredGuideLen, pageH - gridBounds.bottom - edgeInset - guideGap),
  );
  const leftGuideLen = Math.max(
    0,
    Math.min(desiredGuideLen, gridBounds.left - edgeInset - guideGap),
  );
  const rightGuideLen = Math.max(
    0,
    Math.min(desiredGuideLen, pageW - gridBounds.right - edgeInset - guideGap),
  );

  doc.setDrawColor(120);
  doc.setLineWidth(0.01);

  for (const x of xCuts) {
    if (topGuideLen > 0) {
      doc.line(x, edgeInset, x, edgeInset + topGuideLen);
    }
    if (bottomGuideLen > 0) {
      doc.line(x, pageH - edgeInset - bottomGuideLen, x, pageH - edgeInset);
    }
  }

  for (const y of yCuts) {
    if (leftGuideLen > 0) {
      doc.line(edgeInset, y, edgeInset + leftGuideLen, y);
    }
    if (rightGuideLen > 0) {
      doc.line(pageW - edgeInset - rightGuideLen, y, pageW - edgeInset, y);
    }
  }

  doc.setDrawColor(0);
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

function externalUrl(): string {
  return __EXTERNAL_URL__.replace(/\/+$/, "");
}

function recipeUrlFor(recipeId: string): string {
  return `${externalUrl()}/recipes/${recipeId}`;
}

async function generateQrDataUrl(url: string): Promise<string> {
  return QRCode.toDataURL(url, {
    errorCorrectionLevel: "M",
    margin: 1,
    width: 600,
  });
}

// Physical page-flip for duplex printing:
// - Long-edge binding flips around the long edge of the sheet.
// - Short-edge binding flips around the short edge.
// After the flip, one axis of the grid is reversed; we mirror the back cards
// so that the card drawn behind slot (col,row) lands on the physical back of
// that same slot once the sheet is flipped.
function backSlotFlipAxis(
  orientation: "portrait" | "landscape",
  flip: FlipDirection,
): "horizontal" | "vertical" {
  const flipsHorizontally =
    (orientation === "portrait" && flip === "long-edge") ||
    (orientation === "landscape" && flip === "short-edge");
  return flipsHorizontally ? "horizontal" : "vertical";
}

export default function PdfExportModal(props: Props) {
  const [cardW, setCardW] = createSignal(2);
  const [cardH, setCardH] = createSignal(2);
  const [marginXIn, setMarginXIn] = createSignal(0.75);
  const [marginYIn, setMarginYIn] = createSignal(0.75);
  const [gutterIn, setGutterIn] = createSignal(0.15);
  const [borderIn, setBorderIn] = createSignal(0);
  const [orientation, setOrientation] = createSignal<"portrait" | "landscape">(
    "landscape",
  );
  const [showCutGuides, setShowCutGuides] = createSignal(false);
  const [showPageNumbers, setShowPageNumbers] = createSignal(true);
  const [doubleSided, setDoubleSided] = createSignal(false);
  const [flipDirection, setFlipDirection] =
    createSignal<FlipDirection>("long-edge");
  const [backPaddingIn, setBackPaddingIn] = createSignal(BACK_CARD_PAD_DEFAULT);
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
    const mx = marginXIn();
    const my = marginYIn();
    const g = gutterIn();
    const w = cardW();
    const h = cardH();
    const b = borderIn();
    const contentW = pageW - 2 * mx;
    const contentH = pageH - 2 * my;
    const invalid = (reason: string) => ({
      cols: 0,
      rows: 0,
      perPage: 0,
      contentW,
      contentH,
      error: reason,
    });
    if (mx < 0 || my < 0) return invalid("Margins must be non-negative.");
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
      const mY = marginYIn();
      const withPageNums = showPageNumbers();
      const twoSided = doubleSided();
      const frontPageCount = Math.ceil(chosen.length / perPage);
      const totalPhysicalPages = frontPageCount * (twoSided ? 2 : 1);
      const backPad = Math.max(0, backPaddingIn());
      const flipAxis = twoSided
        ? backSlotFlipAxis(orientation(), flipDirection())
        : "horizontal";

      const footW = w + border;
      const footH = h + border;
      const gridW = cols * footW + (cols - 1) * g;
      const gridH = rows * footH + (rows - 1) * g;
      const startX = (pageW - gridW) / 2;
      const startY = (pageH - gridH) / 2;
      const bHalf = border / 2;
      const xCuts = [
        startX,
        ...Array.from(
          { length: cols - 1 },
          (_, col) => startX + col * (footW + g) + footW + g / 2,
        ),
        startX + gridW,
      ];
      const yCuts = [
        startY,
        ...Array.from(
          { length: rows - 1 },
          (_, row) => startY + row * (footH + g) + footH + g / 2,
        ),
        startY + gridH,
      ];

      const pageNumFontPt = 9;
      const pageNumHeightIn = pageNumFontPt / 72;
      const pageNumBaselineY = pageH - Math.max(mY / 2, pageNumHeightIn * 0.75);
      const drawPageNumber = (physicalPageIdx: number, isBack: boolean) => {
        if (!withPageNums) return;
        const sheetIdx = Math.floor(physicalPageIdx / (twoSided ? 2 : 1));
        const sideLabel = twoSided ? (isBack ? "B" : "F") : "";
        const label = twoSided
          ? `${sheetIdx + 1}${sideLabel} / ${frontPageCount} (${physicalPageIdx + 1}/${totalPhysicalPages})`
          : `${physicalPageIdx + 1} / ${totalPhysicalPages}`;
        doc.setFontSize(pageNumFontPt);
        doc.setTextColor(120);
        doc.text(label, pageW / 2, pageNumBaselineY, {
          align: "center",
          baseline: "middle",
        });
        doc.setTextColor(0);
      };

      const drawPageDecorations = (
        physicalPageIdx: number,
        isBack: boolean,
      ) => {
        if (showCutGuides()) {
          drawCutGuides(doc, pageW, pageH, xCuts, yCuts, {
            left: startX,
            right: startX + gridW,
            top: startY,
            bottom: startY + gridH,
          });
        }
        drawPageNumber(physicalPageIdx, isBack);
      };

      const cardRect = (slotCol: number, slotRow: number) => {
        const x = startX + border / 2 + slotCol * (footW + g);
        const y = startY + border / 2 + slotRow * (footH + g);
        return { x, y };
      };

      const drawCardBorder = (x: number, y: number) => {
        if (border > 0) {
          doc.setLineWidth(border);
          doc.setDrawColor(0, 0, 0);
          doc.rect(x - bHalf, y - bHalf, w + border, h + border, "S");
        }
      };

      const drawFrontCard = async (
        recipe: RecipeSummary,
        x: number,
        y: number,
      ) => {
        const pad = 0.05;
        const innerX = x + pad;
        const innerY = y + pad;
        const innerW = w - 2 * pad;
        const innerH = h - 2 * pad;

        const titleAreaH = Math.min(0.45, Math.max(0.25, innerH * 0.22));
        const imgAreaH = innerH - titleAreaH;

        const img: ImageData | null = recipe.thumbnailPhotoId
          ? await cropImageToAspectRatio(
              await fetchThumbnail(recipe.thumbnailPhotoId, tok),
              PHOTO_ASPECT_RATIO,
            )
          : null;

        if (img && imgAreaH > 0) {
          const drawW = Math.min(innerW, imgAreaH * PHOTO_ASPECT_RATIO);
          const drawH = drawW / PHOTO_ASPECT_RATIO;
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
        const lineHeightFactor = doc.getLineHeightFactor();
        let fontPt = 14;
        let lines: string[];
        while (true) {
          doc.setFontSize(fontPt);
          lines = doc.splitTextToSize(recipe.title, innerW) as string[];
          const effLines = Math.min(lines.length, maxLines);
          const neededH = (effLines * fontPt * lineHeightFactor) / 72;
          const fitsWidth = lines.length <= maxLines;
          const fitsHeight = neededH <= titleAreaH;
          if ((fitsWidth && fitsHeight) || fontPt <= 5) break;
          fontPt -= 0.5;
        }
        lines = lines.slice(0, maxLines);
        const titleCenterY = innerY + imgAreaH + titleAreaH / 2;
        doc.text(lines, innerX + innerW / 2, titleCenterY, {
          align: "center",
          baseline: "middle",
        });
      };

      // Mirror the front-card layout so the QR lines up over the photo region
      // on the front (not the title).
      const frontPad = 0.05;
      const frontInnerH = Math.max(0, h - 2 * frontPad);
      const frontTitleAreaH = Math.min(
        0.45,
        Math.max(0.25, frontInnerH * 0.22),
      );
      const frontImgAreaH = Math.max(0, frontInnerH - frontTitleAreaH);
      const frontImgCenterYOffset = frontPad + frontImgAreaH / 2;

      const qrSize = Math.min(
        Math.max(0, w - 2 * backPad),
        Math.max(0, frontImgAreaH * 0.85),
        1.5,
      );

      const descGap = 0.08;
      const pointsPerInch = 72;

      const drawBackCard = async (
        recipe: RecipeSummary,
        x: number,
        y: number,
      ) => {
        if (qrSize <= 0) return;

        // When the physical duplex flip is around a horizontal axis (vertical
        // flipAxis in grid terms), the back content lands upside-down relative
        // to the front after the user flips the sheet. Compensate by rotating
        // the entire back card 180° around its center via a PDF
        // transformation matrix, so we can draw everything at its intended
        // (reader's-perspective) position without fighting jsPDF's text
        // rotation quirks around align/baseline.
        const rotate180 = flipAxis === "vertical";

        // Visual coords (reader's perspective, measured from card top-left).
        const visualQrCenterY = frontImgCenterYOffset;
        const visualTextTopY = visualQrCenterY + qrSize / 2 + descGap;

        if (rotate180) {
          const cx = x + w / 2;
          const cy = y + h / 2;
          // setCurrentTransformationMatrix takes PDF-native coords (points,
          // y-up). Convert from our jsPDF user coords (inches, y-down).
          const cxPts = cx * pointsPerInch;
          const cyPtsPdf = (pageH - cy) * pointsPerInch;
          doc.saveGraphicsState();
          doc.setCurrentTransformationMatrix(
            doc.Matrix(-1, 0, 0, -1, 2 * cxPts, 2 * cyPtsPdf),
          );
        }

        try {
          const qrX = x + (w - qrSize) / 2;
          const qrY = y + visualQrCenterY - qrSize / 2;

          const qrDataUrl = await generateQrDataUrl(recipeUrlFor(recipe.id));
          doc.addImage(qrDataUrl, "PNG", qrX, qrY, qrSize, qrSize);

          const description = (recipe.description ?? "").trim();
          if (!description) return;

          const visualTextAreaH = h - visualTextTopY;
          if (visualTextAreaH <= 0) return;

          const innerW = Math.max(0, w - 2 * backPad);
          const descFontPt = 8;
          doc.setFontSize(descFontPt);
          doc.setTextColor(60);
          const lineHeightIn = (descFontPt * 1.25) / 72;
          const maxLines = Math.max(
            1,
            Math.floor(visualTextAreaH / lineHeightIn),
          );
          let lines = doc.splitTextToSize(description, innerW) as string[];
          if (lines.length > maxLines) {
            lines = lines.slice(0, maxLines);
            const last = lines[maxLines - 1];
            lines[maxLines - 1] =
              last.length > 3 ? `${last.slice(0, -1).trimEnd()}…` : last;
          }

          doc.text(lines, x + w / 2, y + visualTextTopY, {
            align: "center",
            baseline: "top",
          });
          doc.setTextColor(0);
        } finally {
          if (rotate180) {
            doc.restoreGraphicsState();
          }
        }
      };

      let physicalPageIdx = 0;
      for (let pageIdx = 0; pageIdx < frontPageCount; pageIdx++) {
        if (physicalPageIdx > 0) doc.addPage();
        drawPageDecorations(physicalPageIdx, false);

        for (let slot = 0; slot < perPage; slot++) {
          const recipeIdx = pageIdx * perPage + slot;
          if (recipeIdx >= chosen.length) break;
          const recipe = chosen[recipeIdx];
          const col = slot % cols;
          const row = Math.floor(slot / cols);
          const { x, y } = cardRect(col, row);
          drawCardBorder(x, y);
          await drawFrontCard(recipe, x, y);
          if (!twoSided) {
            setProgress({ done: recipeIdx + 1, total: chosen.length });
          }
        }

        physicalPageIdx++;

        if (twoSided) {
          doc.addPage();
          drawPageDecorations(physicalPageIdx, true);

          for (let slot = 0; slot < perPage; slot++) {
            const recipeIdx = pageIdx * perPage + slot;
            if (recipeIdx >= chosen.length) break;
            const recipe = chosen[recipeIdx];
            const col = slot % cols;
            const row = Math.floor(slot / cols);
            const backCol = flipAxis === "horizontal" ? cols - 1 - col : col;
            const backRow = flipAxis === "vertical" ? rows - 1 - row : row;
            const { x, y } = cardRect(backCol, backRow);
            drawCardBorder(x, y);
            await drawBackCard(recipe, x, y);
            setProgress({ done: recipeIdx + 1, total: chosen.length });
          }

          physicalPageIdx++;
        }
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
          <label>Horizontal margin (in)</label>
          <input
            type="number"
            step="0.05"
            min="0"
            value={marginXIn()}
            onInput={(e) =>
              setMarginXIn(parseFloat(e.currentTarget.value) || 0)
            }
          />
        </div>
        <div class="form-group">
          <label>Vertical margin (in)</label>
          <input
            type="number"
            step="0.05"
            min="0"
            value={marginYIn()}
            onInput={(e) =>
              setMarginYIn(parseFloat(e.currentTarget.value) || 0)
            }
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
        <div class="form-group">
          <label>
            <input
              type="checkbox"
              checked={showCutGuides()}
              onChange={(e) => setShowCutGuides(e.currentTarget.checked)}
            />{" "}
            Add cutting guides
          </label>
        </div>
        <div class="form-group">
          <label>
            <input
              type="checkbox"
              checked={showPageNumbers()}
              onChange={(e) => setShowPageNumbers(e.currentTarget.checked)}
            />{" "}
            Show page numbers
          </label>
        </div>
        <div class="form-group">
          <label>
            <input
              type="checkbox"
              checked={doubleSided()}
              onChange={(e) => setDoubleSided(e.currentTarget.checked)}
            />{" "}
            Double-sided (QR code + description on back)
          </label>
        </div>
        <Show when={doubleSided()}>
          <div class="form-group">
            <label>Duplex flip direction</label>
            <select
              value={flipDirection()}
              onChange={(e) =>
                setFlipDirection(e.currentTarget.value as FlipDirection)
              }
            >
              <option value="long-edge">
                Long edge (most printers' default)
              </option>
              <option value="short-edge">Short edge</option>
            </select>
          </div>
          <div class="form-group">
            <label>Back-side padding (in)</label>
            <input
              type="number"
              step="0.05"
              min="0"
              value={backPaddingIn()}
              onInput={(e) =>
                setBackPaddingIn(parseFloat(e.currentTarget.value) || 0)
              }
            />
          </div>
        </Show>

        <p class="recipe-cards-layout-info">
          <Show when={layout().perPage > 0} fallback={<>{layout().error}</>}>
            {layout().cols}×{layout().rows} grid = {layout().perPage} cards per
            page. Photos crop to a fixed 3:2 landscape frame.
            <Show when={doubleSided()}>
              {" "}
              Back pages mirror the grid so each card's back aligns after duplex
              printing with{" "}
              {flipDirection() === "long-edge"
                ? "long-edge"
                : "short-edge"}{" "}
              flipping.
            </Show>
          </Show>
        </p>

        <Show when={error()}>
          <p class="error">{error()}</p>
        </Show>
      </div>
    </Modal>
  );
}
