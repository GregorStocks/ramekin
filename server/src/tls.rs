//! TLS/ACME certificate management for the Ramekin server.
//!
//! When `ACME_ENABLED` is set, this module handles:
//! - Obtaining certificates via Let's Encrypt ACME (with automatic caching)
//! - Serving HTTPS with automatic certificate renewal
//!
//! Certificates are cached in `~/.ramekin/certs/{hostname}/` by the ACME library.

use axum::Router;
use rustls::ServerConfig;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio_rustls_acme::caches::DirCache;
use tokio_rustls_acme::AcmeConfig;
use tokio_stream::StreamExt;

/// Returns the certificate cache directory for a given hostname: `~/.ramekin/certs/{hostname}/`
///
/// Panics if the home directory cannot be determined.
fn cert_cache_dir(hostname: &str) -> PathBuf {
    dirs::home_dir()
        .expect("Could not determine home directory for certificate storage. Set HOME environment variable.")
        .join(".ramekin")
        .join("certs")
        .join(hostname)
}

/// Serve the application with ACME-obtained TLS certificate.
///
/// The ACME library (DirCache) handles:
/// - Checking for existing valid certificates in the cache
/// - Obtaining new certificates when needed
/// - Automatic renewal before expiration
pub async fn serve_with_acme(
    app: Router,
    port: u16,
    hostname: &str,
    email: Option<&str>,
    use_staging: bool,
) {
    let cache_dir = cert_cache_dir(hostname);

    // Ensure the certificate cache directory exists
    if let Err(e) = std::fs::create_dir_all(&cache_dir) {
        panic!(
            "Failed to create certificate cache directory {:?}: {}",
            cache_dir, e
        );
    }

    tracing::info!("ACME certificate cache directory: {:?}", cache_dir);

    // Build the ACME configuration
    let mut config = AcmeConfig::new([hostname]).cache(DirCache::new(cache_dir));

    // Add contact email if provided (recommended by Let's Encrypt for expiry notifications)
    if let Some(email) = email {
        config = config.contact([format!("mailto:{}", email)]);
    }

    // Configure Let's Encrypt environment
    // directory_lets_encrypt(true) = production (trusted certs, lower rate limits)
    // directory_lets_encrypt(false) = staging (untrusted certs, higher rate limits)
    let use_production = !use_staging;
    config = config.directory_lets_encrypt(use_production);

    if use_staging {
        tracing::warn!("Using Let's Encrypt STAGING environment - certificates will NOT be trusted by browsers");
    } else {
        tracing::info!("Using Let's Encrypt PRODUCTION environment");
    }

    let mut acme_state = config.state();

    // Build the rustls ServerConfig with the ACME resolver
    let rustls_config = ServerConfig::builder()
        .with_no_client_auth()
        .with_cert_resolver(acme_state.resolver());

    let acceptor = acme_state.axum_acceptor(Arc::new(rustls_config));

    // Spawn a task to handle ACME events (certificate acquisition/renewal)
    tokio::spawn(async move {
        loop {
            match acme_state.next().await {
                Some(Ok(event)) => {
                    tracing::info!("ACME: {:?}", event);
                }
                Some(Err(err)) => {
                    // Log certificate errors prominently - these are critical for HTTPS to work
                    tracing::error!("ACME certificate error: {:?}", err);
                    tracing::error!(
                        "TLS connections may fail until a valid certificate is obtained. \
                         Check your domain DNS settings and ensure port 443 is accessible."
                    );
                }
                None => {
                    tracing::debug!("ACME event stream ended");
                    break;
                }
            }
        }
    });

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Server listening on https://{}:{}", hostname, port);
    tracing::info!(
        "Swagger UI available at https://{}:{}/swagger-ui/",
        hostname,
        port
    );
    tracing::info!(
        "OpenAPI spec available at https://{}:{}/api-docs/openapi.json",
        hostname,
        port
    );

    axum_server::bind(addr)
        .acceptor(acceptor)
        .serve(app.into_make_service())
        .await
        .expect("Failed to start HTTPS server");
}
