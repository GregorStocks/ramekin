//! Bookmarklet-driven local capture server for the pipeline HTML cache.
//!
//! Some recipe sites (e.g. Just One Cookbook, Serious Eats, The Spruce Eats,
//! Food52) block the pipeline's `reqwest` client at the bot wall, so the URL
//! never reaches `extract_recipe`. This module runs a tiny axum server on
//! localhost: the user opens the page in a real browser session that already
//! passes the bot wall, clicks a bookmarklet, and the rendered HTML is posted
//! back here and written into the same disk cache the pipeline reads from.
//! Subsequent pipeline runs hit cache and skip the network entirely.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use axum::extract::{DefaultBodyLimit, State};
use axum::http::StatusCode;
use axum::response::Html;
use axum::routing::{get, post};
use axum::Router;
use ramekin_core::http::DiskCache;
use serde::Deserialize;
use tower_http::cors::{Any, CorsLayer};
use uuid::Uuid;

/// Default port for the local capture server.
pub const DEFAULT_PORT: u16 = 9876;

/// Max accepted body size for `/capture`. Real-world bot-walled recipe pages
/// (the ones this feature exists for) routinely push 5-15 MB once scripts and
/// inlined assets are included; axum's default 2 MB cap on the `String`
/// extractor would silently 413 the bookmarklet for exactly those URLs.
const MAX_CAPTURE_BODY_BYTES: usize = 64 * 1024 * 1024;

#[derive(Deserialize)]
struct CapturePayload {
    url: String,
    html: String,
    token: String,
}

#[derive(Clone)]
struct AppState {
    cache: Arc<DiskCache>,
    server_url: Arc<String>,
    /// Random per-session token. The bookmarklet served from `/` embeds it,
    /// so any other page hitting `/capture` (which is the only route with
    /// permissive CORS) without it gets 401 instead of being able to poison
    /// the pipeline cache.
    token: Arc<String>,
    /// Optional URL to pin captures to. When set, every capture is written
    /// under this URL instead of the bookmarklet's `location.href`, so the
    /// pipeline (which keys cache lookups by the pre-redirect URL listed in
    /// `test-urls.json`) finds the entry even if the recipe site redirected
    /// the user during browsing.
    target_url: Option<Arc<String>>,
}

/// Run the capture server until the process is interrupted.
pub async fn run(
    host: &str,
    port: u16,
    cache_dir: Option<PathBuf>,
    target_url: Option<String>,
) -> Result<()> {
    let cache_dir = match cache_dir {
        Some(dir) => dir,
        None => resolve_cache_dir_from_env()?,
    };
    std::fs::create_dir_all(&cache_dir)
        .with_context(|| format!("Failed to create cache dir {}", cache_dir.display()))?;

    let server_url = format!("http://{host}:{port}");
    let token = Uuid::new_v4().to_string();
    let state = AppState {
        cache: Arc::new(DiskCache::new(cache_dir.clone())),
        server_url: Arc::new(server_url.clone()),
        token: Arc::new(token),
        target_url: target_url.map(Arc::new),
    };

    let addr: SocketAddr = format!("{host}:{port}")
        .parse()
        .with_context(|| format!("Invalid bind address {host}:{port}"))?;
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("Failed to bind {addr}"))?;

    tracing::info!("");
    tracing::info!("Ramekin pipeline cache capture server");
    tracing::info!("  listening: {server_url}");
    tracing::info!("  cache dir: {}", cache_dir.display());
    tracing::info!("");
    tracing::info!("  1. Open {server_url}/ in your normal browser and drag the bookmarklet");
    tracing::info!("     to your bookmarks bar.");
    tracing::info!(
        "  2. Visit a recipe URL the pipeline can't fetch (anti-bot or rate-limit walled),"
    );
    tracing::info!("     then click the bookmarklet to POST the rendered HTML back here.");
    tracing::info!("  3. Re-run `make pipeline` and the URL will hit cache instead of the wall.");
    tracing::info!("");
    if let Some(target) = &state.target_url {
        tracing::info!("  Captures will be cached under: {target}");
        tracing::info!(
            "  (set via --url; pins the cache key to the URL the pipeline tested in case the"
        );
        tracing::info!("   site redirected your browser to a different canonical URL.)");
    } else {
        tracing::info!("  Captures will be cached under the bookmarklet's location.href.");
        tracing::info!(
            "  If the site redirects your browser, pass --url <pre-redirect-URL> so the cache"
        );
        tracing::info!("  entry matches what `make pipeline` looks up.");
    }
    tracing::info!("");
    tracing::info!(
        "  The bookmarklet embeds a per-session token; re-drag it from / each time you restart."
    );
    tracing::info!("");
    tracing::info!("  Ctrl+C to stop.");

    axum::serve(listener, build_router(state))
        .await
        .map_err(|e| anyhow!("capture server exited: {e}"))?;

    Ok(())
}

fn build_router(state: AppState) -> Router {
    // Permissive CORS only on `/capture` — the bookmarklet POSTs cross-origin
    // from any recipe site. `/` (which serves the bookmarklet + token) MUST
    // stay same-origin so a malicious page can't `fetch('http://localhost:9876/')`,
    // read the embedded token, and forge a capture call.
    let capture_cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Build the two sub-routers separately and merge so router-level layers
    // (CORS, body limit) are scoped to /capture only.
    let capture_router: Router<AppState> = Router::new()
        .route("/capture", post(capture))
        .layer(DefaultBodyLimit::max(MAX_CAPTURE_BODY_BYTES))
        .layer(capture_cors);

    let root_router: Router<AppState> = Router::new().route("/", get(root));

    root_router.merge(capture_router).with_state(state)
}

/// Resolve the HTTP cache directory the same way `CachingClientBuilder` does,
/// so captures land in the same directory the pipeline reads from when the
/// user has set `RAMEKIN_HTTP_CACHE` in `cli.env`.
fn resolve_cache_dir_from_env() -> Result<PathBuf> {
    resolve_cache_dir(std::env::var("RAMEKIN_HTTP_CACHE").ok().as_deref())
}

fn resolve_cache_dir(setting: Option<&str>) -> Result<PathBuf> {
    match setting {
        Some("none") => Err(anyhow!(
            "RAMEKIN_HTTP_CACHE=none disables caching; capture has nowhere to write. \
             Unset it or pass --cache-dir explicitly."
        )),
        Some("disk") | None => Ok(DiskCache::default_dir()),
        Some(path) => Ok(PathBuf::from(path)),
    }
}

async fn root(State(state): State<AppState>) -> Html<String> {
    Html(landing_html(&state.server_url, &state.token))
}

async fn capture(
    State(state): State<AppState>,
    body: String,
) -> Result<String, (StatusCode, String)> {
    // Bookmarklet sends `text/plain` (a CORS "simple" content type, no
    // preflight) with a JSON body, so we parse the body ourselves.
    let payload: CapturePayload = serde_json::from_str(&body)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("invalid JSON body: {e}")))?;

    if payload.token != *state.token {
        return Err((StatusCode::UNAUTHORIZED, "invalid token".to_string()));
    }

    if payload.url.is_empty() || payload.html.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "url and html are both required".to_string(),
        ));
    }

    // If the server was launched with --url, write under that URL so the cache
    // key matches the entry the pipeline looks up (which is the pre-redirect
    // URL listed in test-urls.json). Otherwise key by the bookmarklet's
    // location.href, which is correct for sites that don't redirect.
    let cache_url: &str = state
        .target_url
        .as_ref()
        .map(|s| s.as_str())
        .unwrap_or(&payload.url);

    state
        .cache
        .put(
            cache_url,
            payload.html.as_bytes(),
            Some("text/html".to_string()),
            None,
            None,
        )
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to write cache entry: {e}"),
            )
        })?;

    let bytes = payload.html.len();
    if cache_url == payload.url {
        tracing::info!("captured {} bytes for {}", bytes, cache_url);
        Ok(format!("cached {} bytes for {}", bytes, cache_url))
    } else {
        tracing::info!(
            "captured {} bytes from {} -> cached under {}",
            bytes,
            payload.url,
            cache_url
        );
        Ok(format!(
            "cached {} bytes from {} under {}",
            bytes, payload.url, cache_url
        ))
    }
}

/// Build the bookmarklet's JavaScript source. Kept ASCII-only and free of
/// `"`, `<`, `>`, and `&` so it can be embedded verbatim in an `href` attribute.
fn bookmarklet_js(server_url: &str, token: &str) -> String {
    // server_url comes from our own --host/--port flags; token is a UUID we
    // generated. If a caller does something pathological with quotes the worst
    // case is a broken bookmarklet — strip them defensively.
    let escaped_url = server_url.replace('\'', "");
    let escaped_token = token.replace('\'', "");
    format!(
        "(function(){{\
var u='{escaped_url}/capture';\
var b=JSON.stringify({{url:location.href,html:document.documentElement.outerHTML,token:'{escaped_token}'}});\
function t(m,c){{var d=document.createElement('div');d.textContent=m;d.style.cssText='position:fixed;top:12px;right:12px;z-index:2147483647;background:'+c+';color:#fff;padding:10px 14px;border-radius:6px;font:14px sans-serif;box-shadow:0 4px 12px rgba(0,0,0,.3);';document.body.appendChild(d);setTimeout(function(){{d.remove();}},5000);}}\
fetch(u,{{method:'POST',headers:{{'Content-Type':'text/plain'}},body:b}}).then(function(r){{return r.text();}}).then(function(m){{t(m,'#1f6f3f');}}).catch(function(e){{t('Ramekin error: '+e,'#a00');}});\
}})();"
    )
}

fn landing_html(server_url: &str, token: &str) -> String {
    let bookmarklet = format!("javascript:{}", bookmarklet_js(server_url, token));
    let href = html_escape(&bookmarklet);
    let src = html_escape(&bookmarklet);
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Ramekin pipeline cache capture</title>
<style>
body {{ font: 15px/1.5 -apple-system, system-ui, sans-serif; max-width: 720px; margin: 2rem auto; padding: 0 1rem; color: #222; }}
h1 {{ font-size: 1.4rem; }}
.bookmarklet {{ display: inline-block; padding: 8px 14px; background: #1f6f3f; color: #fff; border-radius: 6px; text-decoration: none; font-weight: 600; }}
code, pre {{ font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 13px; }}
pre {{ background: #f5f5f5; padding: 12px; border-radius: 6px; overflow-x: auto; white-space: pre-wrap; word-break: break-all; }}
ol li {{ margin-bottom: 0.4rem; }}
</style>
</head>
<body>
<h1>Ramekin pipeline cache capture</h1>
<p>Drag this link to your bookmarks bar:</p>
<p><a class="bookmarklet" href="{href}">Capture for Ramekin</a></p>
<ol>
<li>Open a recipe URL the pipeline can't fetch (e.g. Just One Cookbook, Serious Eats, The Spruce Eats, Food52) in a browser session that already passes the bot wall.</li>
<li>Click the bookmarklet. The page's rendered HTML is POSTed to <code>{server_url}/capture</code> and written into the local pipeline cache.</li>
<li>Re-run <code>make pipeline</code>. The URL now hits cache and goes through <code>extract_recipe</code> normally.</li>
</ol>
<p><em>Heads up:</em> the cache is keyed by URL. If the recipe site redirects your browser, the bookmarklet's <code>location.href</code> won't match the URL <code>make pipeline</code> looks up. Restart the server with <code>--url &lt;pre-redirect-URL&gt;</code> to pin captures to the original URL.</p>
<h2>Bookmarklet source</h2>
<pre><code>{src}</code></pre>
</body>
</html>
"#,
        href = href,
        server_url = html_escape(server_url),
        src = src,
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use ramekin_core::http::DiskCache;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tower::ServiceExt;

    const TEST_TOKEN: &str = "test-token-abcdef";

    fn test_state() -> (AppState, TempDir) {
        test_state_with(None)
    }

    fn test_state_with(target_url: Option<String>) -> (AppState, TempDir) {
        let tmp = TempDir::new().unwrap();
        let cache = Arc::new(DiskCache::new(tmp.path().to_path_buf()));
        let state = AppState {
            cache,
            server_url: Arc::new("http://127.0.0.1:9876".to_string()),
            token: Arc::new(TEST_TOKEN.to_string()),
            target_url: target_url.map(Arc::new),
        };
        (state, tmp)
    }

    #[tokio::test]
    async fn capture_writes_html_to_cache() {
        let (state, tmp) = test_state();
        let cache_dir = tmp.path().to_path_buf();
        let app = build_router(state);

        let url = "https://www.justonecookbook.com/example-recipe/";
        let html = "<html><body>example</body></html>";
        let body = serde_json::json!({"url": url, "html": html, "token": TEST_TOKEN}).to_string();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/capture")
                    .header("content-type", "text/plain")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let cache = DiskCache::new(cache_dir);
        let cached = cache.get(url).expect("expected cached response");
        assert_eq!(cached.data, html.as_bytes());
        assert_eq!(cached.metadata.content_type.as_deref(), Some("text/html"));
        assert_eq!(cached.metadata.url, url);
    }

    #[tokio::test]
    async fn capture_rejects_invalid_json() {
        let (state, _tmp) = test_state();
        let app = build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/capture")
                    .body(Body::from("not json"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn capture_rejects_wrong_token() {
        let (state, tmp) = test_state();
        let cache_dir = tmp.path().to_path_buf();
        let app = build_router(state);

        let url = "https://www.example.com/recipe";
        let body = serde_json::json!({
            "url": url,
            "html": "<html></html>",
            "token": "not-the-real-token",
        })
        .to_string();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/capture")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let cache = DiskCache::new(cache_dir);
        assert!(cache.get(url).is_none(), "cache must not be poisoned");
    }

    #[tokio::test]
    async fn capture_rejects_empty_fields() {
        let (state, _tmp) = test_state();
        let app = build_router(state);

        let body = serde_json::json!({"url": "", "html": "", "token": TEST_TOKEN}).to_string();
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/capture")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn capture_target_url_overrides_payload_url() {
        let target = "https://www.justonecookbook.com/recipe-original/";
        let (state, tmp) = test_state_with(Some(target.to_string()));
        let cache_dir = tmp.path().to_path_buf();
        let app = build_router(state);

        let bookmarklet_url = "https://justonecookbook.com/recipe-redirected";
        let body = serde_json::json!({
            "url": bookmarklet_url,
            "html": "<html>r</html>",
            "token": TEST_TOKEN,
        })
        .to_string();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/capture")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let cache = DiskCache::new(cache_dir);
        assert!(
            cache.get(target).is_some(),
            "cache must be keyed by --url target"
        );
        assert!(
            cache.get(bookmarklet_url).is_none(),
            "cache must not be keyed by location.href when target is set"
        );
    }

    #[tokio::test]
    async fn root_has_no_cors_allow_origin() {
        // The bookmarklet+token must not be readable cross-origin, otherwise
        // any tab can fetch /, scrape the token, and forge captures.
        let (state, _tmp) = test_state();
        let app = build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header("origin", "https://evil.example.com")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            response
                .headers()
                .get("access-control-allow-origin")
                .is_none(),
            "/ must not advertise cross-origin readability"
        );
    }

    #[tokio::test]
    async fn capture_has_cors_allow_origin() {
        // The bookmarklet legitimately fetches /capture cross-origin, so this
        // route must advertise CORS so the browser exposes the response body.
        let (state, _tmp) = test_state();
        let app = build_router(state);

        let body = serde_json::json!({
            "url": "https://example.com/r",
            "html": "<html></html>",
            "token": TEST_TOKEN,
        })
        .to_string();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/capture")
                    .header("origin", "https://recipe.example.com")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert!(response
            .headers()
            .get("access-control-allow-origin")
            .is_some());
    }

    #[tokio::test]
    async fn capture_accepts_large_body() {
        // Real bot-walled recipe pages routinely exceed the 2 MB axum default;
        // verify a 5 MB body still goes through.
        let (state, tmp) = test_state();
        let cache_dir = tmp.path().to_path_buf();
        let app = build_router(state);

        let url = "https://www.seriouseats.com/big-page";
        let html = "x".repeat(5 * 1024 * 1024);
        let body = serde_json::json!({"url": url, "html": html, "token": TEST_TOKEN}).to_string();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/capture")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let cache = DiskCache::new(cache_dir);
        assert!(cache.get(url).is_some());
    }

    #[test]
    fn resolve_cache_dir_matches_pipeline_semantics() {
        assert_eq!(resolve_cache_dir(None).unwrap(), DiskCache::default_dir());
        assert_eq!(
            resolve_cache_dir(Some("disk")).unwrap(),
            DiskCache::default_dir()
        );
        assert_eq!(
            resolve_cache_dir(Some("/custom/cache")).unwrap(),
            PathBuf::from("/custom/cache")
        );
        assert!(resolve_cache_dir(Some("none")).is_err());
    }

    #[tokio::test]
    async fn root_serves_bookmarklet_page() {
        let (state, _tmp) = test_state();
        let app = build_router(state);

        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
            .await
            .unwrap();
        let html = std::str::from_utf8(&bytes).unwrap();
        assert!(html.contains("Capture for Ramekin"));
        assert!(html.contains("javascript:"));
        assert!(html.contains("/capture"));
        assert!(
            html.contains(TEST_TOKEN),
            "bookmarklet must embed the per-session token"
        );
    }
}
