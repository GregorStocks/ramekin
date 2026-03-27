## 2026-03-26 Generated Client Failure

Running `make test` and `make lint` currently fails before the actual test/lint steps complete.

Observed path:

- `make test`
- `make lint`
- both build `api/openapi.json`
- both then fail in `scripts/generate-clients.sh` while compiling `ramekin-ui/generated-client`

Current failure mode:

- TypeScript compile errors in generated files such as `generated-client/apis/RecipesApi.ts`
- imports from `../models/index` no longer resolve for many generated symbols
- `generated-client/apis/index.ts` also references files like `./AuthApi` and `./EnrichApi` that are not present under the generated output

This appears to be repo/tooling drift in client generation rather than anything specific to the iOS nutritional-info fix.
