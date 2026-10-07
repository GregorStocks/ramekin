---
name: audit-ui
description: Bring up the local Ramekin dev server, drive the UI with playwright-cli, and capture screenshots/snapshots for any kind of UI audit (design critique, bug repro, UX review, regression check).
---

# Audit the UI Locally

Use this when you need to *see* the running app — design critique, bug repro,
UX review, screenshot gathering. It covers the setup path; *what* you audit
is up to the task that invoked this skill.

## 1. Start the dev server

```
make dev-headless
```

Run it in the background (it does not exit). On a fresh worktree the Rust
release build can take 5-10 minutes the first time; subsequent runs are
fast. `db-up` and client-codegen are dependencies of this target and run
automatically.

The ports live in `dev.env` and are **per-worktree** — don't hardcode them.
Pull them with:

```
grep -E '^(PORT|UI_PORT)=' dev.env
```

- `PORT` is the API (plain HTTP).
- `UI_PORT` is the Vite dev server. **It serves HTTPS with a self-signed
  cert**, so use `https://localhost:$UI_PORT/` and pass `-k` to curl.

Wait for the UI by polling `https://localhost:$UI_PORT/` until it returns
200. Don't start a chain of `sleep`s — use `Monitor` with an `until` loop,
or `Bash` with `run_in_background: true` and wait for the notification.

## 2. Log in

There is a test user `t` / `t` created by `make seed`. If a prior session
already seeded the DB it still exists (the DB is persistent across `make
dev-down`). A valid token looks like a string of 64 `t`s — you can grab it
with:

```
curl -sS -X POST http://localhost:$PORT/api/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"t","password":"t"}'
```

If login fails, the seed is missing. Note: `make seed` in this repo
currently errors with "missing --server-url" — rather than fixing that
ad-hoc, escalate or work around by calling `cd cli && cargo run -q -- seed
--server-url http://localhost:$PORT --username t --password t
../data/dev/seed.paprikarecipes` — but confirm with the user first, since
the project rule is "always use existing Makefile commands."

## 3. Drive the browser

Use the `playwright-cli` skill (invoke it once to load its help into
context, then run commands directly).

```
playwright-cli open --browser=chrome
playwright-cli resize 1920 1080
playwright-cli goto https://localhost:$UI_PORT/
```

The dev cert triggers no prompt in chromium — the page loads directly. If a
goto hangs, it's a compile/crash upstream, not a cert issue.

### Logging in via the UI

From the login page snapshot, grab the `ref`s for the username/password
fields and the submit button:

```
playwright-cli fill e10 "t"
playwright-cli fill e13 "t"
playwright-cli click e14
```

Refs change on every snapshot — always re-read the latest `.playwright-cli/
page-*.yml` to get current ones. Snapshot files are small YAML; prefer them
over screenshots for "what's on the page."

### Screenshots

```
playwright-cli screenshot --filename=cookbook-1920.png
```

Then `Read` the PNG so you can actually see it. For anything involving
layout density / fluid grids, **screenshot at least two widths** — 1920
(normal desktop) and 2560 (ultrawide). Plenty of layouts look fine at
1200px and fall apart on big displays.

For mobile/narrow checks, `playwright-cli resize 390 844` (iPhone 14 Pro)
or `768 1024` (tablet).

## 4. When you're done

```
playwright-cli close
```

The dev server stays up — don't tear it down unless the user asked. If you
do need to stop it, `make dev-down` kills the process-compose processes
without touching Postgres; `make db-down` stops the DB.

## Gotchas

- **HTTPS, not HTTP**, for the UI. `curl` returns HTTP 000 on plain HTTP
  because the port is terminated by TLS.
- **Per-worktree ports.** Don't bake 63819 (or any number) into the skill
  or the task — always read from `dev.env`.
- **First compile is slow.** Use `ScheduleWakeup` for long pre-known waits
  (drop to 270s max to stay in cache, or commit to 20+ min). Use Monitor
  with an until-loop to get notified the moment the UI is reachable.
- **Snapshots > screenshots** for reading structure; screenshots only when
  you need to see pixels (design work, spacing, visual regression).
- **Look at more than one viewport width** whenever density, responsive
  behavior, or "does this scale?" is in scope.
- **Don't use `make dev`** — it needs a TTY for process-compose's TUI and
  hangs when run from a tool. Always `make dev-headless`.
