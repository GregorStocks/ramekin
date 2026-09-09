# Recipe UI design iteration

Audit performed on 2026-09-08 using the seeded local app and Chromium.
The web app supplies a warm light theme; an OS dark preference should keep
that palette readable rather than partially recoloring native controls.

## Findings and changes

| Finding | Change |
| --- | --- |
| Navigation link rules override New Recipe's white text, producing dark text on brown (2.37:1 contrast). | Exclude button links from navigation text overrides. The primary action now measures 5.04:1, and 6.03:1 on hover. |
| Ten recipe actions stretch into tall, uneven buttons; Edit's label sits above its neighbors. | Use consistent centered link/button metrics, keep Edit directly visible, and group the remaining actions in a keyboard-operable More actions disclosure. Keep slow operations visible so their progress labels remain readable. |
| Maintenance controls and version history precede the recipe title. | Lead with the recipe, give ingredients and instructions consistent headings, and move history below the reading layout. |
| Mobile sort and density controls shrink until their labels are clipped. | Wrap whole controls and retain the density group's intrinsic width. Align the search field with the desktop toolbar. |
| Custom scale reads “Cus” because the shared input padding overrides its sizing. | Give the field sufficient usable width and an accessible name. |
| Ingredient editing stacks every field separately on phones; remove controls are faint and small. | Pair amount/unit below the name on narrow screens, enlarge removal targets, and give icon actions and inputs accessible names. |
| Photo upload cannot be reached by keyboard because its input is hidden inside a label. | Open the file chooser from a real, focusable button. |
| Form focus outlines are removed by more specific component rules. | Preserve keyboard focus outlines. Give disabled buttons a distinct, readable state and keep the save bar opaque. |
| iOS Edit is buried in the overflow menu; ingredient fields use fixed 50/60-point widths. | Expose Edit in the toolbar, label creation/overflow actions, and give ingredient names a full row with flexible amount/unit fields and larger removal targets. |
| CI screenshots show low-contrast iOS scale/login actions, and the UI test measures an 18.7-point accessibility frame despite 44-point layout padding. | Use an adaptive amber accent with inverse text on selected controls, test light/dark contrast, and explicitly define rectangular interaction shapes for padded ingredient actions. |

## Before / after

Matching viewports and the same seeded recipe make the layout changes comparable.

| Screen | Before | After |
| --- | --- | --- |
| Cookbook, 1920 × 1080 | [Before](before-cookbook-1920.png) | [After](after-cookbook-1920.png) |
| Cookbook, 2560 × 1440 | [Before](before-cookbook-2560.png) | [After](after-cookbook-2560.png) |
| Cookbook, 390 × 844 | [Before](before-cookbook-390.png) | [After](after-cookbook-390.png) |
| Recipe, 1920 × 1080 | [Before](before-detail-1920.png) | [After](after-detail-1920.png) |
| Recipe, 390 × 844 | [Before](before-detail-390.png) | [After](after-detail-390.png) |
| Edit, 1920 × 1080 | [Before](before-edit-1920.png) | [After](after-edit-1920.png) |
| Edit, 390 × 844 | [Before](before-edit-390.png) | [After](after-edit-390.png) |
| Create, 390 × 844 | [Before](before-create-390.png) | [After](after-create-390.png) |

iOS screenshots are attached to the existing `testRecipeFlow` test results,
now uploaded on success as well as failure by the macOS UI test job. The
simulator runs on macOS; its exported screenshots were downloaded and
reviewed on this Linux host.

## Validation

Local checks passed: `make test` (400 API tests plus the Rust suites),
`make ui-unit-test` (132 tests), `make test-ui` (53 tests), and `make lint`.

Browser regression coverage exercises 320, 390, 768, 1920, and 2560-pixel
widths, light/dark OS preferences, computed text contrast, unclipped density
labels, aligned actions, keyboard disclosure operation, photo chooser access,
and recipe creation/edit/save. Existing photo generation and shopping-list
scaling tests follow the revised action placement.

No deterministic client logic or API contract changed. The separate
`p2-paste-recipe-text-for-backend-parsing` issue owns the requested paste-first
creation flow; these form styles also apply to editing an imported draft.

Follow-ups discovered during the audit are tracked in
`p3-web-modal-keyboard-focus`, `p3-ui-dependency-audit-findings`, and
`p3-dev-seed-enrichment-auth-failures`.
