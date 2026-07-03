# Shopping-List Keyboard Fix + Client Log Upload Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the slow keyboard on the iOS shopping-list add flow (with before/after latency measurement) and add a client-log upload pipeline (server endpoint + upload buttons on iOS and web) so client performance can be diagnosed from uploaded logs.

**Architecture:** New soft-deleted `client_log_uploads` table with three authenticated user-scoped endpoints under `/api/client-logs` (utoipa-annotated; all four API clients regenerate automatically). iOS uploads its existing `DebugLogger` file via the hand-written `RamekinAPI` layer; web gets a new ring-buffer logger uploaded via the generated TypeScript client. The iOS keyboard fix defers the `@FocusState` write until the `TextField` exists, and perf marks (tap → `keyboardDidShowNotification`) land first so the fix has before/after numbers.

**Tech Stack:** Rust (axum, Diesel DSL, utoipa), Postgres, Python pytest e2e tests, SolidJS + Vitest, SwiftUI.

**Spec:** `docs/superpowers/specs/2026-07-02-shopping-keyboard-and-client-logs-design.md`

## Global Constraints

- Everything runs via existing Makefile targets — never raw `cargo`/`docker`/system Python/NPM. New-need targets require asking the user first.
- Diesel DSL only — no raw SQL anywhere (including no `diesel::dsl::sql` fragments).
- Soft deletes only: `deleted_at TIMESTAMPTZ`, never hard-delete rows.
- Fail fast; no silent fallbacks. `tracing::` macros for Rust logging, never `println!`/`eprintln!`.
- Never hand-edit generated code: `api/openapi.json`, `server/src/schema.rs`, `cli/generated/`, `ramekin-ui/generated-client/`, `tests/generated/`, `ramekin-ios/generated-client/`.
- No linter bypasses; Python imports only at top of file.
- No backwards-compatibility shims.
- `make lint` must pass before the PR.
- No `make pipeline` rerun needed — this feature does not touch extraction/parsing.
- Web TS: `strict`, `noUnusedLocals`, `verbatimModuleSyntax` (use `import type`), no enums (`erasableSyntaxOnly`), Prettier style (double quotes, 2-space indent, semicolons, trailing commas).
- Commit after every task. Commit messages end with `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`.

---

### Task 1: Migration, schema, models for `client_log_uploads`

**Files:**
- Create: `migrations/2026-07-02-000000-0000_create_client_log_uploads/up.sql`
- Create: `migrations/2026-07-02-000000-0000_create_client_log_uploads/down.sql`
- Modify: `server/src/schema.rs` (via `make generate-schema` only — never by hand)
- Modify: `server/src/models.rs` (append at end of file)

**Interfaces:**
- Produces: Diesel table `crate::schema::client_log_uploads`; structs `crate::models::ClientLogUpload` (Queryable) and `crate::models::NewClientLogUpload<'a>` (Insertable) used by Task 2.

- [ ] **Step 1: Write the migration**

`migrations/2026-07-02-000000-0000_create_client_log_uploads/up.sql`:

```sql
CREATE TABLE client_log_uploads (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    platform TEXT NOT NULL,
    app_version TEXT,
    os_info TEXT,
    content TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ
);

CREATE INDEX idx_client_log_uploads_user_id ON client_log_uploads(user_id) WHERE deleted_at IS NULL;
```

`migrations/2026-07-02-000000-0000_create_client_log_uploads/down.sql`:

```sql
DROP TABLE client_log_uploads;
```

(No `updated_at`: uploads are immutable records; the only mutation ever allowed is soft delete.)

- [ ] **Step 2: Regenerate schema.rs**

Run: `make generate-schema`
Expected: `server/src/schema.rs` gains a `diesel::table! { client_log_uploads (id) { ... } }` block, a `diesel::joinable!(client_log_uploads -> users (user_id));` line, and `client_log_uploads` in `allow_tables_to_appear_in_same_query!`. `git diff server/src/schema.rs` shows only those additions.

- [ ] **Step 3: Add models**

Append to `server/src/models.rs` (field order MUST match the column order in the new `schema.rs` block):

```rust
// Client log uploads: debug logs uploaded from the iOS/web clients for diagnostics
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = crate::schema::client_log_uploads)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[allow(dead_code)]
pub struct ClientLogUpload {
    pub id: Uuid,
    pub user_id: Uuid,
    pub platform: String,
    pub app_version: Option<String>,
    pub os_info: Option<String>,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::client_log_uploads)]
pub struct NewClientLogUpload<'a> {
    pub user_id: Uuid,
    pub platform: &'a str,
    pub app_version: Option<&'a str>,
    pub os_info: Option<&'a str>,
    pub content: &'a str,
}
```

(`Uuid`, `DateTime`, `Utc` are already imported at the top of `models.rs`.)

- [ ] **Step 4: Verify it compiles via lint**

Run: `make lint`
Expected: passes (the new structs are `#[allow(dead_code)]` until Task 2 uses them).

- [ ] **Step 5: Commit**

```bash
git add migrations/2026-07-02-000000-0000_create_client_log_uploads server/src/schema.rs server/src/models.rs
git commit -m "Add client_log_uploads table and models"
```

---

### Task 2: `/api/client-logs` endpoints (POST, GET list, GET by id)

**Files:**
- Create: `server/src/api/client_logs/mod.rs`
- Create: `server/src/api/client_logs/create.rs`
- Create: `server/src/api/client_logs/list.rs`
- Create: `server/src/api/client_logs/get.rs`
- Modify: `server/src/api/mod.rs` (module declaration + `openapi()` modules Vec)
- Modify: `server/src/main.rs` (nest router under `protected_router`, near the other `.nest(...)` calls around lines 234-253)

**Interfaces:**
- Consumes: `ClientLogUpload` / `NewClientLogUpload` from Task 1; existing `AuthUser`, `ApiError`, `ErrorResponse`, `get_conn!`, `DbPool`, `AppState`.
- Produces: handlers `create_client_log`, `list_client_logs`, `get_client_log` with request/response schemas `CreateClientLogRequest`, `CreateClientLogResponse`, `ListClientLogsResponse`, `ClientLogSummary`, `GetClientLogResponse`. Generated clients (Task 3+) expose these as `ClientLogsApi` with methods named after the handler functions (Python: `create_client_log`; TypeScript: `createClientLog({ createClientLogRequest })`, `listClientLogs()`, `getClientLog({ id })`).

- [ ] **Step 1: Write `create.rs`**

`server/src/api/client_logs/create.rs`:

```rust
use crate::api::{ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::models::NewClientLogUpload;
use crate::schema::client_log_uploads;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

/// Matches the iOS DebugLogger rotation cap; a full log file always fits.
pub const MAX_CONTENT_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateClientLogRequest {
    pub platform: String,
    pub app_version: Option<String>,
    pub os_info: Option<String>,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CreateClientLogResponse {
    pub id: Uuid,
}

#[utoipa::path(
    post,
    path = "/api/client-logs",
    tag = "client_logs",
    request_body = CreateClientLogRequest,
    responses(
        (status = 201, description = "Log upload stored", body = CreateClientLogResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 413, description = "Content too large", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub async fn create_client_log(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Json(request): Json<CreateClientLogRequest>,
) -> impl IntoResponse {
    if request.platform != "ios" && request.platform != "web" {
        return ApiError::invalid_request("platform must be \"ios\" or \"web\"").into_response();
    }
    if request.content.is_empty() {
        return ApiError::invalid_request("content must not be empty").into_response();
    }
    if request.content.len() > MAX_CONTENT_BYTES {
        return ApiError::payload_too_large(format!(
            "content exceeds maximum size of {MAX_CONTENT_BYTES} bytes"
        ))
        .into_response();
    }

    let mut conn = get_conn!(pool);
    let new_upload = NewClientLogUpload {
        user_id: user.id,
        platform: &request.platform,
        app_version: request.app_version.as_deref(),
        os_info: request.os_info.as_deref(),
        content: &request.content,
    };
    let id = match diesel::insert_into(client_log_uploads::table)
        .values(&new_upload)
        .returning(client_log_uploads::id)
        .get_result::<Uuid>(&mut conn)
    {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("Failed to insert client log upload: {e}");
            return ApiError::internal("Failed to store log upload").into_response();
        }
    };

    (StatusCode::CREATED, Json(CreateClientLogResponse { id })).into_response()
}
```

- [ ] **Step 2: Write `list.rs`**

`server/src/api/client_logs/list.rs`:

```rust
use crate::api::{ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::models::ClientLogUpload;
use crate::schema::client_log_uploads;
use axum::{extract::State, response::IntoResponse, Json};
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::Serialize;
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ClientLogSummary {
    pub id: Uuid,
    pub platform: String,
    pub app_version: Option<String>,
    pub os_info: Option<String>,
    pub created_at: DateTime<Utc>,
    pub content_length: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ListClientLogsResponse {
    pub uploads: Vec<ClientLogSummary>,
}

#[utoipa::path(
    get,
    path = "/api/client-logs",
    tag = "client_logs",
    responses(
        (status = 200, description = "Caller's log uploads, newest first", body = ListClientLogsResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_client_logs(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
) -> impl IntoResponse {
    let mut conn = get_conn!(pool);
    let rows: Vec<ClientLogUpload> = match client_log_uploads::table
        .filter(client_log_uploads::user_id.eq(user.id))
        .filter(client_log_uploads::deleted_at.is_null())
        .order(client_log_uploads::created_at.desc())
        .select(ClientLogUpload::as_select())
        .load(&mut conn)
    {
        Ok(rows) => rows,
        Err(e) => {
            tracing::error!("Failed to list client log uploads: {e}");
            return ApiError::internal("Failed to list log uploads").into_response();
        }
    };

    let uploads = rows
        .into_iter()
        .map(|row| ClientLogSummary {
            id: row.id,
            platform: row.platform,
            app_version: row.app_version,
            os_info: row.os_info,
            created_at: row.created_at,
            content_length: row.content.len() as i64,
        })
        .collect();

    Json(ListClientLogsResponse { uploads }).into_response()
}
```

(Note: content length is computed in Rust from the loaded row rather than via a SQL `char_length` — Diesel has no built-in DSL for it and raw SQL fragments are banned. Row counts here are tiny.)

- [ ] **Step 3: Write `get.rs`**

`server/src/api/client_logs/get.rs`:

```rust
use crate::api::{ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::get_conn;
use crate::models::ClientLogUpload;
use crate::schema::client_log_uploads;
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::Serialize;
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct GetClientLogResponse {
    pub id: Uuid,
    pub platform: String,
    pub app_version: Option<String>,
    pub os_info: Option<String>,
    pub created_at: DateTime<Utc>,
    pub content: String,
}

#[utoipa::path(
    get,
    path = "/api/client-logs/{id}",
    tag = "client_logs",
    params(("id" = Uuid, Path, description = "Log upload id")),
    responses(
        (status = 200, description = "Full log upload", body = GetClientLogResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Not found", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_client_log(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let mut conn = get_conn!(pool);
    let row: Option<ClientLogUpload> = match client_log_uploads::table
        .filter(client_log_uploads::id.eq(id))
        .filter(client_log_uploads::user_id.eq(user.id))
        .filter(client_log_uploads::deleted_at.is_null())
        .select(ClientLogUpload::as_select())
        .first(&mut conn)
        .optional()
    {
        Ok(row) => row,
        Err(e) => {
            tracing::error!("Failed to fetch client log upload: {e}");
            return ApiError::internal("Failed to fetch log upload").into_response();
        }
    };

    let Some(row) = row else {
        return ApiError::not_found("Log upload not found").into_response();
    };

    Json(GetClientLogResponse {
        id: row.id,
        platform: row.platform,
        app_version: row.app_version,
        os_info: row.os_info,
        created_at: row.created_at,
        content: row.content,
    })
    .into_response()
}
```

- [ ] **Step 4: Write `mod.rs` and register the module**

`server/src/api/client_logs/mod.rs`:

```rust
pub mod create;
pub mod get;
pub mod list;

use crate::AppState;
use axum::extract::DefaultBodyLimit;
use axum::routing::get;
use axum::Router;
use utoipa::OpenApi;

/// Body limit above MAX_CONTENT_BYTES so the in-handler check produces the
/// precise 413 message for oversized `content`; this layer only backstops
/// pathological bodies.
const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/",
            get(list::list_client_logs)
                .post(create::create_client_log)
                .layer(DefaultBodyLimit::max(MAX_BODY_BYTES)),
        )
        .route("/{id}", get(get::get_client_log))
}

#[derive(OpenApi)]
#[openapi(
    paths(create::create_client_log, list::list_client_logs, get::get_client_log),
    components(schemas(
        create::CreateClientLogRequest,
        create::CreateClientLogResponse,
        list::ClientLogSummary,
        list::ListClientLogsResponse,
        get::GetClientLogResponse
    ))
)]
pub struct ApiDoc;
```

In `server/src/api/mod.rs`: add `pub mod client_logs;` to the module declarations (keep alphabetical order with the existing `pub mod ...;` lines) and add `client_logs::ApiDoc::openapi()` to the `modules` Vec inside `openapi()` (around lines 39-51).

In `server/src/main.rs`, inside `protected_router` (with the other `.nest` calls, around lines 234-253), add in alphabetical position:

```rust
.nest("/api/client-logs", api::client_logs::router())
```

- [ ] **Step 5: Verify compile + regenerated spec**

Run: `make lint`
Expected: passes. Then run `git status` — `api/openapi.json` and generated client dirs may show diffs (regenerated by the build); that is expected and they get committed as-is. Verify with `grep -c client-logs api/openapi.json` → non-zero.

- [ ] **Step 6: Commit**

```bash
git add server/src/api/client_logs server/src/api/mod.rs server/src/main.rs api/openapi.json
git add -A cli/generated ramekin-ui/generated-client tests/generated ramekin-ios/generated-client
git commit -m "Add /api/client-logs endpoints for client debug log upload"
```

(If the generated dirs are gitignored, `git add -A` on them is a no-op — check `git status` first and only add what's tracked.)

---

### Task 3: E2E tests for `/api/client-logs`

**Files:**
- Create: `tests/test_client_logs.py`

**Interfaces:**
- Consumes: generated Python `ClientLogsApi` (from Task 2's utoipa annotations; regenerated automatically by `make test`), fixtures `authed_api_client`, `second_authed_api_client`, `unauthed_api_client` from `tests/conftest.py`.

- [ ] **Step 1: Write the tests**

`tests/test_client_logs.py`:

```python
"""E2E tests for the /api/client-logs endpoints."""

import pytest
from ramekin_client.api import ClientLogsApi
from ramekin_client.exceptions import ApiException
from ramekin_client.models import CreateClientLogRequest


def test_client_log_round_trip(authed_api_client):
    client, _user_id = authed_api_client
    api = ClientLogsApi(client)

    first = api.create_client_log(
        CreateClientLogRequest(
            platform="ios",
            app_version="1.0.0",
            os_info="iOS 19.0",
            content="line one\nline two\n",
        )
    )
    second = api.create_client_log(
        CreateClientLogRequest(platform="web", content="web log line\n")
    )

    listing = api.list_client_logs()
    # Newest first
    assert [u.id for u in listing.uploads] == [second.id, first.id]

    summary = listing.uploads[1]
    assert summary.platform == "ios"
    assert summary.app_version == "1.0.0"
    assert summary.os_info == "iOS 19.0"
    assert summary.content_length == len("line one\nline two\n")

    fetched = api.get_client_log(first.id)
    assert fetched.content == "line one\nline two\n"
    assert fetched.platform == "ios"


def test_client_log_user_scoping(authed_api_client, second_authed_api_client):
    client, _user_id = authed_api_client
    other_client, _other_user_id = second_authed_api_client

    created = ClientLogsApi(client).create_client_log(
        CreateClientLogRequest(platform="web", content="private logs")
    )

    other_api = ClientLogsApi(other_client)
    assert other_api.list_client_logs().uploads == []
    with pytest.raises(ApiException) as exc_info:
        other_api.get_client_log(created.id)
    assert exc_info.value.status == 404


def test_client_log_requires_auth(unauthed_api_client):
    api = ClientLogsApi(unauthed_api_client)
    with pytest.raises(ApiException) as exc_info:
        api.list_client_logs()
    assert exc_info.value.status == 401
    with pytest.raises(ApiException) as exc_info:
        api.create_client_log(CreateClientLogRequest(platform="web", content="x"))
    assert exc_info.value.status == 401


def test_client_log_rejects_bad_platform(authed_api_client):
    client, _user_id = authed_api_client
    with pytest.raises(ApiException) as exc_info:
        ClientLogsApi(client).create_client_log(
            CreateClientLogRequest(platform="android", content="x")
        )
    assert exc_info.value.status == 400


def test_client_log_rejects_empty_content(authed_api_client):
    client, _user_id = authed_api_client
    with pytest.raises(ApiException) as exc_info:
        ClientLogsApi(client).create_client_log(
            CreateClientLogRequest(platform="web", content="")
        )
    assert exc_info.value.status == 400


def test_client_log_rejects_oversized_content(authed_api_client):
    client, _user_id = authed_api_client
    too_big = "x" * (2 * 1024 * 1024 + 1)
    with pytest.raises(ApiException) as exc_info:
        ClientLogsApi(client).create_client_log(
            CreateClientLogRequest(platform="web", content=too_big)
        )
    assert exc_info.value.status == 413
```

Adjust only if the regenerated client's names differ (check `tests/generated/ramekin_client/api/` after `make test` regenerates; the API class comes from `tag = "client_logs"` and methods from the handler fn names). If `uploads[].id` deserializes as `str` rather than UUID, the equality assertions still hold since both sides come from the client.

- [ ] **Step 2: Run the suite**

Run: `make test`
Expected: all tests pass, including the 6 new ones in `tests/test_client_logs.py`. If `ClientLogsApi` import fails, the clients didn't regenerate — run `make clean-api` then `make test` again.

- [ ] **Step 3: Commit**

```bash
git add tests/test_client_logs.py
git add -A tests/generated  # only if tracked and diffed
git commit -m "Add e2e tests for /api/client-logs"
```

---

### Task 4: Web ring-buffer logger (TDD)

**Files:**
- Create: `ramekin-ui/src/utils/logger.ts`
- Test: `ramekin-ui/src/utils/logger.test.ts`

**Interfaces:**
- Produces: `logger` object with `log(source, message)`, `warn(source, message)`, `error(source, message)`, `timed<T>(source, label, fn)`, `dump(): string`, `entries(): readonly LogEntry[]`, `clear()`. Consumed by Tasks 5 and 6.

- [ ] **Step 1: Write the failing tests**

`ramekin-ui/src/utils/logger.test.ts`:

```ts
import { beforeEach, describe, expect, it } from "vitest";
import { logger } from "./logger";

describe("logger", () => {
  beforeEach(() => {
    logger.clear();
  });

  it("records entries with level, source, and message", () => {
    logger.log("Shopping", "hello");
    logger.warn("Import", "careful");
    logger.error("Capture", "boom");

    const entries = logger.entries();
    expect(entries).toHaveLength(3);
    expect(entries[0]).toMatchObject({
      level: "log",
      source: "Shopping",
      message: "hello",
    });
    expect(entries[1].level).toBe("warn");
    expect(entries[2].level).toBe("error");
    // ISO-8601 timestamp
    expect(new Date(entries[0].timestamp).toISOString()).toBe(
      entries[0].timestamp,
    );
  });

  it("evicts the oldest entries beyond capacity", () => {
    for (let i = 0; i < 1005; i++) {
      logger.log("Test", `entry ${i}`);
    }
    const entries = logger.entries();
    expect(entries).toHaveLength(1000);
    expect(entries[0].message).toBe("entry 5");
    expect(entries[999].message).toBe("entry 1004");
  });

  it("timed logs start and completion and returns the result", async () => {
    const result = await logger.timed("Shopping", "createItems", async () => 42);
    expect(result).toBe(42);

    const entries = logger.entries();
    expect(entries).toHaveLength(2);
    expect(entries[0].message).toBe("createItems started");
    expect(entries[1].message).toMatch(/^createItems completed \(\d+ms\)$/);
  });

  it("timed logs failure with elapsed time and rethrows", async () => {
    await expect(
      logger.timed("Shopping", "createItems", async () => {
        throw new Error("nope");
      }),
    ).rejects.toThrow("nope");

    const entries = logger.entries();
    expect(entries).toHaveLength(2);
    expect(entries[1].level).toBe("error");
    expect(entries[1].message).toMatch(
      /^createItems FAILED after \d+ms: Error: nope$/,
    );
  });

  it("dump formats one line per entry", () => {
    logger.log("Shopping", "first");
    logger.error("Capture", "second");

    const lines = logger.dump().split("\n");
    expect(lines).toHaveLength(2);
    expect(lines[0]).toMatch(
      /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z \[log\] \[Shopping\] first$/,
    );
    expect(lines[1]).toMatch(/\[error\] \[Capture\] second$/);
  });

  it("dump returns an empty string when there are no entries", () => {
    expect(logger.dump()).toBe("");
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `make ui-unit-test`
Expected: FAIL — `logger.test.ts` cannot resolve `./logger`.

- [ ] **Step 3: Implement the logger**

`ramekin-ui/src/utils/logger.ts`:

```ts
export type LogLevel = "log" | "warn" | "error";

export interface LogEntry {
  timestamp: string;
  level: LogLevel;
  source: string;
  message: string;
}

/** Mirrors the iOS DebugLogger's shape: timestamped source-tagged lines. */
const MAX_ENTRIES = 1000;

const buffer: LogEntry[] = [];

function record(level: LogLevel, source: string, message: string): void {
  buffer.push({
    timestamp: new Date().toISOString(),
    level,
    source,
    message,
  });
  if (buffer.length > MAX_ENTRIES) {
    buffer.splice(0, buffer.length - MAX_ENTRIES);
  }
  console[level](`[${source}] ${message}`);
}

export const logger = {
  log(source: string, message: string): void {
    record("log", source, message);
  },

  warn(source: string, message: string): void {
    record("warn", source, message);
  },

  error(source: string, message: string): void {
    record("error", source, message);
  },

  /** Logs start/completion (with elapsed ms) around an async operation. */
  async timed<T>(
    source: string,
    label: string,
    fn: () => Promise<T>,
  ): Promise<T> {
    record("log", source, `${label} started`);
    const start = performance.now();
    try {
      const result = await fn();
      const elapsed = Math.round(performance.now() - start);
      record("log", source, `${label} completed (${elapsed}ms)`);
      return result;
    } catch (err) {
      const elapsed = Math.round(performance.now() - start);
      record("error", source, `${label} FAILED after ${elapsed}ms: ${String(err)}`);
      throw err;
    }
  },

  /** Formats the buffer as text lines for upload. */
  dump(): string {
    return buffer
      .map((e) => `${e.timestamp} [${e.level}] [${e.source}] ${e.message}`)
      .join("\n");
  },

  entries(): readonly LogEntry[] {
    return buffer.slice();
  },

  clear(): void {
    buffer.length = 0;
  },
};
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `make ui-unit-test`
Expected: PASS (all logger tests plus existing suites).

- [ ] **Step 5: Commit**

```bash
git add ramekin-ui/src/utils/logger.ts ramekin-ui/src/utils/logger.test.ts
git commit -m "Add web ring-buffer debug logger"
```

---

### Task 5: Instrument web shopping-list path; convert console call sites

**Files:**
- Modify: `ramekin-ui/src/pages/ShoppingListPage.tsx` (loadItems lines ~51-67, handleAddItem lines ~73-95)
- Modify: `ramekin-ui/src/pages/CapturePage.tsx` (console.error at lines 38, 52, 70)
- Modify: `ramekin-ui/src/pages/ImportPage.tsx` (console.warn at lines 65, 148-151)

**Interfaces:**
- Consumes: `logger` from Task 4 (`import { logger } from "../utils/logger";`).

- [ ] **Step 1: Instrument ShoppingListPage**

Add the import, then wrap the API calls in `logger.timed`. `loadItems` becomes:

```ts
  const loadItems = async (showLoading = true) => {
    if (showLoading) setLoading(true);
    setError(null);
    try {
      const response = await logger.timed("Shopping", "listItems", () =>
        getShoppingListApi().listItems(),
      );
      setItems(response.items);
      setCategoryOrder(response.categoryOrder);
    } catch (err) {
      const message = await extractApiError(
        err,
        "Failed to load shopping list",
      );
      setError(message);
    } finally {
      setLoading(false);
    }
  };
```

`handleAddItem` becomes:

```ts
  const handleAddItem = async () => {
    const name = newItemName().trim();
    if (!name) return;

    setAdding(true);
    setError(null);
    try {
      const amount = newItemAmount().trim() || undefined;
      await logger.timed("Shopping", "createItems", () =>
        getShoppingListApi().createItems({
          createShoppingListRequest: {
            items: [{ item: name, amount }],
          },
        }),
      );
      setNewItemName("");
      setNewItemAmount("");
      await loadItems(false);
    } catch (err) {
      const message = await extractApiError(err, "Failed to add item");
      setError(message);
    } finally {
      setAdding(false);
    }
  };
```

(`timed` already logs failures; the catch blocks keep their user-facing error handling unchanged.)

- [ ] **Step 2: Convert CapturePage and ImportPage**

In `CapturePage.tsx`, add `import { logger } from "../utils/logger";` and replace:
- Line 38: `console.error("[Ramekin Capture] No token found in localStorage");` → `logger.error("Capture", "No token found in localStorage");`
- Line 52: `console.error("[Ramekin Capture] API error:", err);` → `logger.error("Capture", \`API error: ${String(err)}\`);`
- Line 70: `console.error("[Ramekin Capture] No token found - user not logged in");` → `logger.error("Capture", "No token found - user not logged in");`

In `ImportPage.tsx`, add the same import and replace:
- Line 65: `console.warn("Error polling scrape job; retrying", err);` → `logger.warn("Import", \`Error polling scrape job; retrying: ${String(err)}\`);`
- Lines 148-151: `console.warn(\`Photo upload failed for recipe "${recipe.name}"; continuing without it\`, err,);` → `logger.warn("Import", \`Photo upload failed for recipe "${recipe.name}"; continuing without it: ${String(err)}\`);`

- [ ] **Step 3: Verify**

Run: `make ui-unit-test` (expect PASS) and `make lint` (expect pass — this also type-checks the UI via the client marker build).

- [ ] **Step 4: Commit**

```bash
git add ramekin-ui/src/pages/ShoppingListPage.tsx ramekin-ui/src/pages/CapturePage.tsx ramekin-ui/src/pages/ImportPage.tsx
git commit -m "Instrument web shopping-list path with debug logger"
```

---

### Task 6: Web upload button on SettingsPage

**Files:**
- Modify: `ramekin-ui/src/context/AuthContext.tsx` (add `ClientLogsApi` getter alongside the existing `getXApi()` factories, lines ~95-103)
- Modify: `ramekin-ui/src/pages/SettingsPage.tsx` (new Diagnostics section)

**Interfaces:**
- Consumes: generated `ClientLogsApi` from `ramekin-client` (exists after Task 2's regeneration); `logger` from Task 4; `extractApiError` from `../utils/recipeFormHelpers` (already imported in SettingsPage).
- Produces: `getClientLogsApi()` on the auth context, following the exact pattern of the existing getters.

- [ ] **Step 1: Add the API getter**

In `AuthContext.tsx`: add `ClientLogsApi` to the existing `ramekin-client` import list, add a `getClientLogsApi` factory next to the others (same shape: `const getClientLogsApi = () => new ClientLogsApi(getAuthedConfig());`), and expose it in the context value object alongside the existing getters (mirror `getShoppingListApi` everywhere it appears, including the context type).

- [ ] **Step 2: Add the Diagnostics section to SettingsPage**

In `SettingsPage.tsx`: pull `getClientLogsApi` from `useAuth()` (line ~12), add signals and a handler:

```ts
  type UploadState = "idle" | "uploading" | "done";
  const [uploadState, setUploadState] = createSignal<UploadState>("idle");
  const [uploadError, setUploadError] = createSignal<string | null>(null);

  const handleUploadLogs = async () => {
    setUploadState("uploading");
    setUploadError(null);
    logger.log("Settings", "uploading debug logs");
    try {
      await getClientLogsApi().createClientLog({
        createClientLogRequest: {
          platform: "web",
          osInfo: navigator.userAgent,
          content: logger.dump(),
        },
      });
      setUploadState("done");
    } catch (err) {
      setUploadState("idle");
      setUploadError(await extractApiError(err, "Failed to upload logs"));
    }
  };
```

(The `logger.log` line before `dump()` guarantees the content is never empty, which the server rejects.)

Add the section after the existing settings sections, matching their markup (`Show` is already available from solid-js or add to imports):

```tsx
      <section class="settings-section">
        <h3>Diagnostics</h3>
        <p>
          Upload this session's debug logs to the server so performance
          problems can be investigated.
        </p>
        <button
          type="button"
          class="btn btn-small"
          disabled={uploadState() === "uploading"}
          onClick={handleUploadLogs}
        >
          {uploadState() === "uploading" ? "Uploading…" : "Upload debug logs"}
        </button>
        <Show when={uploadState() === "done"}>
          <p>Logs uploaded.</p>
        </Show>
        <Show when={uploadError()}>
          <p class="error">{uploadError()}</p>
        </Show>
      </section>
```

Match the file's existing status/error markup conventions (check how the connection-status section renders errors and reuse its classes; use `Switch`/`Match` instead of `Show` if that's more consistent with the surrounding code).

Also add `import { logger } from "../utils/logger";`.

- [ ] **Step 3: Verify**

Run: `make lint` (type-checks the UI). Expected: pass. If `createClientLog` / `ClientLogsApi` names don't resolve, inspect `ramekin-ui/generated-client/apis/` for the actual generated names (derived from the utoipa `tag` and handler fn names) and use those — do not edit the generated code.

- [ ] **Step 4: Manual smoke test (optional if dev server already running)**

Run: `make dev-headless`, log in at the local UI, visit Settings, click "Upload debug logs", confirm "Logs uploaded." appears. Then stop with `make dev-down`.

- [ ] **Step 5: Commit**

```bash
git add ramekin-ui/src/context/AuthContext.tsx ramekin-ui/src/pages/SettingsPage.tsx
git commit -m "Add debug log upload button to web settings"
```

---

### Task 7: iOS `RamekinAPI.uploadLogs`

**Files:**
- Create: `ramekin-ios/Shared/RamekinAPI+ClientLogs.swift` (picked up automatically by xcodegen — no project.yml change)

**Interfaces:**
- Consumes: `RamekinAPI.performRequest(method:path:body:acceptedStatusCodes:)` (internal helper in `Shared/RamekinAPI.swift`, same usage as `RamekinAPI+Exports.swift`).
- Produces: `RamekinAPI.shared.uploadLogs(_ content: String) async throws` used by Task 8.

- [ ] **Step 1: Write the extension**

`ramekin-ios/Shared/RamekinAPI+ClientLogs.swift`:

```swift
import Foundation
import UIKit

extension RamekinAPI {
    private struct UploadLogsRequestBody: Encodable {
        let platform: String
        let appVersion: String?
        let osInfo: String?
        let content: String

        enum CodingKeys: String, CodingKey {
            case platform
            case appVersion = "app_version"
            case osInfo = "os_info"
            case content
        }
    }

    /// Uploads the DebugLogger contents to the server for diagnostics.
    func uploadLogs(_ content: String) async throws {
        let info = Bundle.main.infoDictionary
        let version = info?["CFBundleShortVersionString"] as? String
        let build = info?["CFBundleVersion"] as? String
        let appVersion = [version, build].compactMap { $0 }.joined(separator: " ")

        let body = try JSONEncoder().encode(UploadLogsRequestBody(
            platform: "ios",
            appVersion: appVersion.isEmpty ? nil : appVersion,
            osInfo: "iOS \(UIDevice.current.systemVersion)",
            content: content
        ))
        _ = try await performRequest(
            method: "POST",
            path: "/api/client-logs",
            body: body,
            acceptedStatusCodes: [201]
        )
    }
}
```

Check `performRequest`'s exact signature in `Shared/RamekinAPI.swift` (lines ~229-291) before writing the call — match parameter names/defaults exactly as `RamekinAPI+Exports.swift` does. If `Shared/` files must also compile for the share extension target and `UIDevice` causes trouble there, keep the file in `Shared/` only if it builds; otherwise move it to `ramekin-ios/Ramekin/` (app target only) — the Settings screen is the only caller.

- [ ] **Step 2: Build**

Run: `make ios-build`
Expected: build succeeds.

- [ ] **Step 3: Commit**

```bash
git add ramekin-ios/Shared/RamekinAPI+ClientLogs.swift
git commit -m "Add RamekinAPI.uploadLogs for client log upload"
```

---

### Task 8: iOS upload button in Settings

**Files:**
- Modify: `ramekin-ios/Ramekin/SettingsView.swift` (Debug section, lines ~141-151; state vars near lines 10-12)

**Interfaces:**
- Consumes: `RamekinAPI.shared.uploadLogs(_:)` from Task 7; `DebugLogger.shared.readLogs()`; `RamekinAPI.APIError`.

- [ ] **Step 1: Add state and button**

Add state vars next to the existing ones:

```swift
    @State private var isUploadingLogs = false
    @State private var logUploadError: String?
    @State private var showingLogUploadSuccess = false
```

In the `Section("Debug")`, after the "View Debug Logs" button, add:

```swift
    Button {
        Task { await uploadLogs() }
    } label: {
        HStack {
            Label("Upload Logs to Server", systemImage: "icloud.and.arrow.up")
            Spacer()
            if isUploadingLogs {
                ProgressView()
            }
        }
    }
    .disabled(isUploadingLogs)
```

Add the worker following the `exportAllRecipes` pattern (lines ~210-227):

```swift
    @MainActor
    private func uploadLogs() async {
        guard !isUploadingLogs else { return }
        isUploadingLogs = true
        defer { isUploadingLogs = false }

        let content = DebugLogger.shared.readLogs()
        guard !content.isEmpty else {
            logUploadError = "No logs to upload"
            return
        }

        do {
            try await RamekinAPI.shared.uploadLogs(content)
            showingLogUploadSuccess = true
        } catch let apiError as RamekinAPI.APIError {
            logUploadError = apiError.errorDescription ?? "Upload failed"
        } catch {
            logUploadError = error.localizedDescription
        }
    }
```

Add alerts near the view's existing presentation modifiers (mirror the `.alert` idiom used by `ExportPresentationModifier` in `ShareSheet.swift` — an error alert driven by a `Binding` over the optional string, plus a success alert):

```swift
    .alert("Logs Uploaded", isPresented: $showingLogUploadSuccess) {
        Button("OK", role: .cancel) {}
    }
    .alert(
        "Upload Failed",
        isPresented: Binding(
            get: { logUploadError != nil },
            set: { if !$0 { logUploadError = nil } }
        )
    ) {
        Button("OK", role: .cancel) {}
    } message: {
        Text(logUploadError ?? "")
    }
```

- [ ] **Step 2: Build**

Run: `make ios-build`
Expected: build succeeds.

- [ ] **Step 3: Commit**

```bash
git add ramekin-ios/Ramekin/SettingsView.swift
git commit -m "Add Upload Logs to Server button in iOS settings"
```

---

### Task 9: iOS keyboard-latency perf marks (lands BEFORE the fix)

**Files:**
- Modify: `ramekin-ios/Ramekin/ShoppingListView.swift`

**Interfaces:**
- Consumes: `DebugLogger.shared.log(_:source:)`. Log line shape matches the existing `[Source] message` convention with source `"Shopping"`.
- Produces: log lines `add tapped` and `keyboard shown +<N>ms after add tap` — these are the before/after evidence for Task 10 (numbers go in the PR description).

- [ ] **Step 1: Add the marks**

At the top of the file add `import UIKit` (for `UIResponder`). Add state:

```swift
    @State private var addTapTime: CFAbsoluteTime?
```

Change the "+" button action (lines ~24-29) to record the tap (keeping the existing same-frame focus behavior for now — Task 10 changes it):

```swift
    Button {
        addTapTime = CFAbsoluteTimeGetCurrent()
        DebugLogger.shared.log("add tapped", source: "Shopping")
        isAddingItem = true
        addFieldFocused = true
    } label: {
        Image(systemName: "plus")
    }
```

Add alongside the existing `.onChange`/`.refreshable` modifiers on the NavigationStack content:

```swift
    .onReceive(
        NotificationCenter.default.publisher(
            for: UIResponder.keyboardDidShowNotification
        )
    ) { _ in
        if let tapTime = addTapTime {
            let elapsedMs = Int((CFAbsoluteTimeGetCurrent() - tapTime) * 1000)
            DebugLogger.shared.log(
                "keyboard shown +\(elapsedMs)ms after add tap",
                source: "Shopping"
            )
            addTapTime = nil
        }
    }
```

- [ ] **Step 2: Build and capture the BEFORE number**

Run: `make ios-build`
Expected: build succeeds. If a simulator or device is at hand, run the app, tap "+" on the Shopping List a few times (dismiss the section with Done between taps), then read the `keyboard shown +Nms` lines via Settings → View Debug Logs. Record the numbers for the PR description. If no simulator is available in this environment, note in the PR that the before/after numbers come from the user's device via the new Upload Logs button.

- [ ] **Step 3: Commit**

```bash
git add ramekin-ios/Ramekin/ShoppingListView.swift
git commit -m "Add tap-to-keyboard latency logging on shopping list"
```

---

### Task 10: iOS keyboard focus fix

**Files:**
- Modify: `ramekin-ios/Ramekin/ShoppingListView.swift`

**Interfaces:**
- Consumes: the perf marks from Task 9 (unchanged).

- [ ] **Step 1: Defer focus until the field exists**

Change the "+" button action to stop setting focus on a not-yet-mounted field:

```swift
    Button {
        addTapTime = CFAbsoluteTimeGetCurrent()
        DebugLogger.shared.log("add tapped", source: "Shopping")
        if isAddingItem {
            addFieldFocused = true
        } else {
            isAddingItem = true
        }
    } label: {
        Image(systemName: "plus")
    }
```

In `addItemSection`, give the Ingredient field an `.onAppear` (this is what focuses it the first time, once it actually exists in the hierarchy):

```swift
    TextField("Ingredient", text: $ingredientName)
        .focused($addFieldFocused)
        .submitLabel(.done)
        .onSubmit(addItem)
        .onAppear {
            addFieldFocused = true
        }
```

`addItem()`'s existing `addFieldFocused = true` re-focus (line ~174) stays as is — the field is already mounted there.

- [ ] **Step 2: Build and capture the AFTER number**

Run: `make ios-build`
Expected: build succeeds. Same measurement procedure as Task 9 Step 2; the `keyboard shown +Nms` values should drop noticeably. Record for the PR description.

- [ ] **Step 3: Commit**

```bash
git add ramekin-ios/Ramekin/ShoppingListView.swift
git commit -m "Focus shopping-list add field only after it mounts"
```

---

### Task 11: Final verification

**Files:** none new.

- [ ] **Step 1: Full test suite**

Run: `make test`
Expected: everything passes (API tests incl. `test_client_logs.py`, Rust tests, UI unit tests).

- [ ] **Step 2: Lint**

Run: `make lint`
Expected: clean.

- [ ] **Step 3: iOS build + tests**

Run: `make ios-build` then `make ios-test`
Expected: both succeed.

- [ ] **Step 4: Working tree check**

Run: `git status`
Expected: clean, or only diffs that belong to this work (e.g. regenerated clients/spec — commit them; per repo policy never silently exclude unexplained diffs — escalate if anything unexpected appears).

- [ ] **Step 5: Commit any remaining generated-artifact diffs**

```bash
git add -A && git commit -m "Regenerate API clients for client-logs endpoints"
```

(Only if Step 4 showed tracked generated-file diffs.)
