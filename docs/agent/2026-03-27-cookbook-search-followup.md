## 2026-03-27 Follow-up Notes

- `scripts/generate-clients.sh` was vulnerable to concurrent invocations. Running `make lint` and `make test` at the same time could corrupt `cli/generated/ramekin-client` and `ramekin-ui/generated-client` because both jobs remove and replace the same output directories.
- `make test-ui` was also broken in worktrees with local UI certs. Vite served HTTPS on `UI_PORT`, but the process-compose readiness probe and Playwright base URL both used plain HTTP on that same port, so `ui-tests` stayed skipped. The fix was to export `UI_PORT_HTTP` in `scripts/run-ui-tests.sh` and point the compose health check plus `UI_BASE_URL` at Vite's HTTP mirror.
