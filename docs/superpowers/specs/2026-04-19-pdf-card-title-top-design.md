# PDF card export: title on top, QR aligned under image

## Motivation

Moving the recipe title to the top of the front card is a design preference.
With the image now at the bottom, the QR on the back needs to move too — we
want it to sit directly behind the photo so dark ink stacks on dark ink and
prevents bleedthrough. (Putting the QR anywhere else would let it show
through the lighter title region and vice versa.)

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

Mirror the new front layout in reader coords so the QR aligns through the
paper with the front image:

- `visualQrCenterY = frontPad + frontTitleAreaH + frontImgAreaH / 2` — the
  same Y center as the front image. Under the existing `rotate180`
  compensation, the QR lands in the correct physical position on the back
  after a duplex flip.
- Description fills the band *above* the QR (reader coords from `backPad` to
  `visualQrCenterY - qrSize/2 - descGap`), behind the front title region.
- Description is capped at **3 lines**, with the area-based cap still
  respected: `min(3, floor(visualTextAreaH / lineHeightIn))`.
- Short descriptions anchor to the bottom of the text band (just above the
  QR) so they don't float near the top edge.

## Non-goals

- No sizing or font changes beyond the line-count cap.
- No changes to back-side rotation logic for horizontal duplex.
- No changes to the modal UI.
