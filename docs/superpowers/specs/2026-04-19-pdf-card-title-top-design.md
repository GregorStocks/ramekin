# PDF card export: title on top, QR relocated

## Motivation

Moving the recipe title to the top of the front card is a design preference.
Keeping the QR code centered over the image region (where it is today) would
cause bleedthrough on printed cards because the QR on the back would sit
directly behind the photo on the front. Put the QR on the opposite end of the
card from the image.

## Front card (`drawFrontCard`)

Swap title and image regions:

- Title band on top, sized with the same formula as today
  (`min(0.45, max(0.25, innerH * 0.22))`).
- Image area fills the remaining inner height below the title.
- Title centering uses `innerY + titleAreaH / 2` instead of
  `innerY + imgAreaH + titleAreaH / 2`.
- Image `drawY` is computed from the image-area top, which is now
  `innerY + titleAreaH` instead of `innerY`.
- Placeholder rectangle + emoji follow the image region.

## Back card (`drawBackCard`)

Mirror the new front layout so QR and image are on opposite ends:

- `visualQrCenterY` moves to the top half of the card. Compute it to align
  vertically with the front's image region inverted: the front image now spans
  `[frontPad + frontTitleAreaH, h - frontPad]`; the back QR center should
  align with where the front title currently sits, i.e.
  `frontPad + frontTitleAreaH / 2` — but placed so the QR square is
  comfortably within the top region. Concretely: QR center at
  `frontPad + frontTitleAreaH / 2` (shifted down if needed so the top of the
  QR respects `backPad`).
- Description moves to the bottom. Its top is
  `visualQrCenterY + qrSize / 2 + descGap` as before; because the QR is now
  up top, this puts the description in the lower portion of the card.
- Description is limited to **at most 3 lines** regardless of available space
  (still respect the area-based cap — the final limit is
  `min(3, floor(visualTextAreaH / lineHeightIn))`).

## Non-goals

- No sizing or font changes beyond the line-count cap.
- No changes to back-side rotation logic for horizontal duplex.
- No changes to the modal UI.
