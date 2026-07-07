import QRCode from "qrcode";

declare const __EXTERNAL_URL__: string;

export type PdfImageData = {
  dataUrl: string;
  format: "JPEG" | "PNG";
  pxW: number;
  pxH: number;
};

export type PdfFlipDirection = "long-edge" | "short-edge";

export type PdfCropRegion = {
  sourceX: number;
  sourceY: number;
  sourceW: number;
  sourceH: number;
};

export type CutGuideDocument = {
  setDrawColor: (...channels: number[]) => void;
  setLineWidth: (width: number) => void;
  line: (x1: number, y1: number, x2: number, y2: number) => void;
};

export const PDF_PHOTO_ASPECT_RATIO = 3 / 2;
export const PDF_BACK_CARD_PAD_DEFAULT = 0.1;

export async function blobToDataUrl(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(blob);
  });
}

export async function getImagePixelSize(
  dataUrl: string,
): Promise<{ w: number; h: number }> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve({ w: img.naturalWidth, h: img.naturalHeight });
    img.onerror = () => reject(new Error("image decode failed"));
    img.src = dataUrl;
  });
}

export function cropRegionForAspectRatio(
  img: Pick<PdfImageData, "pxW" | "pxH">,
  targetRatio: number,
): PdfCropRegion | null {
  if (targetRatio <= 0) return null;

  const sourceRatio = img.pxW / img.pxH;
  if (Math.abs(sourceRatio - targetRatio) < 0.01) return null;

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

  return { sourceX, sourceY, sourceW, sourceH };
}

export async function cropImageToAspectRatio(
  img: PdfImageData,
  targetRatio: number,
): Promise<PdfImageData> {
  const crop = cropRegionForAspectRatio(img, targetRatio);
  if (!crop) return img;

  const image = new Image();
  image.src = img.dataUrl;
  await image.decode();

  const canvas = document.createElement("canvas");
  canvas.width = crop.sourceW;
  canvas.height = crop.sourceH;

  const ctx = canvas.getContext("2d");
  if (!ctx) {
    throw new Error("Failed to create canvas context for image crop");
  }

  ctx.drawImage(
    image,
    crop.sourceX,
    crop.sourceY,
    crop.sourceW,
    crop.sourceH,
    0,
    0,
    crop.sourceW,
    crop.sourceH,
  );

  const mimeType = img.format === "PNG" ? "image/png" : "image/jpeg";
  return {
    dataUrl: canvas.toDataURL(mimeType, 0.92),
    format: img.format,
    pxW: crop.sourceW,
    pxH: crop.sourceH,
  };
}

export function drawCutGuides(
  doc: CutGuideDocument,
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

export async function fetchThumbnail(
  photoId: string,
  token: string,
  authedFetch: (
    input: RequestInfo | URL,
    init?: RequestInit,
  ) => Promise<Response>,
): Promise<PdfImageData> {
  const maxAttempts = 4;
  let lastErr: unknown;
  for (let attempt = 1; attempt <= maxAttempts; attempt++) {
    try {
      const resp = await authedFetch(
        `/api/photos/${photoId}/thumbnail?size=1200`,
        {
          headers: { Authorization: `Bearer ${token}` },
        },
      );
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

export function externalUrl(): string {
  return __EXTERNAL_URL__.replace(/\/+$/, "");
}

export function recipeUrlFor(recipeId: string): string {
  return `${externalUrl()}/recipes/${recipeId}`;
}

export async function generateQrDataUrl(url: string): Promise<string> {
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
export function backSlotFlipAxis(
  orientation: "portrait" | "landscape",
  flip: PdfFlipDirection,
): "horizontal" | "vertical" {
  const flipsHorizontally =
    (orientation === "portrait" && flip === "long-edge") ||
    (orientation === "landscape" && flip === "short-edge");
  return flipsHorizontally ? "horizontal" : "vertical";
}
