# Design: iOS Share Extension Captures Page HTML Like the Web Bookmarklet

## Problem

The iOS share extension currently POSTs a bare URL to `/api/scrape`, which
triggers a backend `fetch` step. This fails for any site that requires
authentication (e.g. America's Test Kitchen) because the backend does not have
the user's browser cookies. Meanwhile, the web bookmarklet already solves this:
it grabs the logged-in page's rendered HTML in-browser and POSTs
`{html, source_url}` to `/api/scrape/capture`, bypassing the backend fetch.

iOS has no analogue of a bookmarklet, but Safari's share extension mechanism
offers the equivalent: a JavaScript preprocessing file that runs in the page's
context before handoff to the extension, with access to the rendered DOM and
Safari's session cookies.

## Goals

- iOS share extension sends page HTML to `/api/scrape/capture`, the same
  endpoint the web bookmarklet hits.
- Logged-in pages (ATK, NYT Cooking, etc.) work the first time.
- Mechanism mirrors the bookmarklet: capture rendered DOM in the page context,
  POST `{html, source_url}` to the capture endpoint.

## Non-goals

- Making non-Safari share sources (Messages, Mail, Notes) work. The only way to
  read Safari's cookies is via a Safari-hosted JS preprocessing file, so the
  extension becomes Safari-only by construction.
- Changing the backend. `/api/scrape/capture` already accepts
  `{html, source_url}` and is used by the web bookmarklet today
  (`server/src/api/scrape/capture.rs`).
- Changing the in-app rescrape flow (`RecipeDetailViewRescrape.swift`), which
  continues to use the existing URL-only path.
- The iOS "paste a URL" in-app flow (no such flow exists today; if added later
  it will necessarily be URL-only since non-extension iOS code cannot read
  Safari cookies).

## Architecture

```
Safari tab (logged in to ATK)
  └─> user taps Share -> Ramekin
        └─> SharePreprocessor.js runs in page context
              (reads document.documentElement.outerHTML + location.href)
        └─> iOS hands preprocessed payload to ShareViewController via
              NSItemProvider with kUTTypePropertyList
        └─> ShareViewController calls RamekinAPI.shared.captureHTML(...)
        └─> POST /api/scrape/capture  { html, source_url }
        └─> backend creates capture job, same flow as web bookmarklet
```

## Changes

### 1. `RamekinShareExtension/Info.plist`

Under `NSExtension.NSExtensionAttributes`:

- **Remove** `NSExtensionActivationSupportsWebURLWithMaxCount`. URL-only
  activations (from Messages, Mail, etc.) cannot produce page HTML, and we do
  not want to silently fall back to the cookie-less backend fetch.
- **Keep** `NSExtensionActivationSupportsWebPageWithMaxCount = 1`. This is what
  makes the extension appear in Safari's share sheet.
- **Add** `NSExtensionJavaScriptPreprocessingFile = SharePreprocessor`. iOS
  resolves this to `SharePreprocessor.js` in the extension bundle.

### 2. `RamekinShareExtension/SharePreprocessor.js` (new)

```js
var ExtensionPreprocessingJS = {
  run: function (args) {
    args.completionFunction({
      html: document.documentElement.outerHTML,
      url: location.href,
      title: document.title
    });
  }
};
```

Must be added to the share-extension target's **Copy Bundle Resources** build
phase (Xcode project file edit).

### 3. `RamekinShareExtension/ShareViewController.swift`

Replace the URL-extraction path with a preprocessed-payload extraction path.

- Drop `extractURL`/`SharedURLExtractor` usage.
- New helper (in a new file `Shared/SharedPagePayloadExtractor.swift`) reads
  the first `NSItemProvider` on the extension item that conforms to
  `UTType.propertyList`, calls `loadItem(forTypeIdentifier: kUTTypePropertyList)`,
  and pulls `NSExtensionJavaScriptPreprocessingResultsKey` out of the returned
  dictionary. That dictionary is `{html, url, title}` from the JS file.
- Define a `SharedPagePayload { html: String; url: URL; title: String? }` struct
  in the same file.
- `presentShareView` now takes a `SharedPagePayload?` rather than `URL?`.
- `ShareExtensionView.sendURL` becomes `sendCapture(_ payload: SharedPagePayload)`
  and calls `RamekinAPI.shared.captureHTML(...)` (new method; see #4).
- If no payload arrives (e.g. user somehow triggers the extension without JS
  results), show the existing error state. Do not fall back to URL-only —
  fail fast, matching the project's "never fail gracefully" rule.

Existing UX (status states, slow-affordance, cancel, retry) is preserved.
Existing `slowAffordanceDelay = 10s` and 15s request timeout remain.

### 4. `Shared/RamekinAPI.swift`

Add a new method and request/response types; keep `scrapeURL` (still used by
`RescrapeAPI` via the backend rescrape endpoint).

```swift
struct CaptureRequest: Encodable {
    let html: String
    let source_url: String
}

func captureHTML(html: String, sourceURL: String) async throws -> ScrapeResponse {
    let body = try JSONEncoder().encode(CaptureRequest(html: html, source_url: sourceURL))
    let data = try await performRequest(
        method: "POST",
        path: "/api/scrape/capture",
        body: body,
        timeoutInterval: Self.scrapeSubmitTimeout
    )
    return try JSONDecoder().decode(ScrapeResponse.self, from: data)
}
```

(The endpoint returns the same `CreateScrapeResponse` shape as `/api/scrape`,
so `ScrapeResponse` is reused.)

### 5. `Shared/SharedURLExtractor.swift`

Leave in place for now. It is still used by the pre-existing URL share path if
any other call site references it; search confirms only `ShareViewController`
used it, so after this change it is unused. Delete it in the implementation
plan.

### 6. OpenAPI / generated client

`server/src/api/scrape/capture.rs` already registers the route in OpenAPI, and
the generated iOS client already exposes `ScrapeAPI.capture` (see
`ramekin-ios/generated-client/docs/ScrapeAPI.md:7`). This design uses the
hand-rolled `RamekinAPI` layer to match how the existing share extension calls
the backend — we do not introduce the generated client into the share
extension target.

## Testing

### Unit tests

- **`RamekinTests/RamekinAPITests.swift`** — add `testCaptureRequestEncoding`
  mirroring the existing `testScrapeRequestEncoding`, plus a `URLProtocol`-
  stubbed round-trip test for `captureHTML` asserting the POST body and path.
- **`RamekinTests/SharedPagePayloadExtractorTests.swift`** (new) — exercise the
  extractor against a fake `NSItemProvider` returning a
  `[NSExtensionJavaScriptPreprocessingResultsKey: [html:..., url:..., title:...]]`
  dictionary. Cover: happy path, missing keys, wrong types, empty HTML.
- **`RamekinShareExtensionTests`** (existing, added in commit `3e444cf8f`) —
  update share-extension coverage to the new payload-shaped contract (payload
  present / absent / captureHTML failure).

### Manual smoke test

1. Log in to ATK in Safari on device.
2. Share → Ramekin. Confirm the capture job succeeds and the recipe imports
   with the real logged-in content (not a paywall page).
3. Share from Messages (share a link to yourself). Confirm Ramekin no longer
   appears in the share sheet (Safari-only by design).

## Risks / open questions

- **Payload size.** Rendered HTML can be large (hundreds of KB to a few MB).
  iOS extensions have ~120 MB memory and ~30 s runtime limits; no payload size
  limit on the extension side, and `/api/scrape/capture` already accepts the
  web bookmarklet's full-page HTML without issue. No change needed, but worth
  watching in logs.
- **Xcode project file edit.** Adding `SharePreprocessor.js` to the share
  extension target requires modifying `Ramekin.xcodeproj/project.pbxproj`.
  The implementation plan should call this out explicitly — it's the easy
  thing to forget.
- **Drop of URL activation is a behavior change.** Users who shared from
  Messages will no longer see Ramekin in the share sheet. Acceptable per the
  non-goals, and matches the explicit user direction ("work like a
  bookmarklet").
