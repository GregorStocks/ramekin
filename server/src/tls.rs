//! TLS/ACME certificate management for the Ramekin server.
//!
//! When `ACME_ENABLED` is set, this module handles:
//! - Checking for existing valid certificates in `~/.ramekin/certs/{hostname}/`
//! - Obtaining new certificates via Let's Encrypt ACME
//! - Serving HTTPS with automatic certificate renewal

use axum::Router;
use axum_server::tls_rustls::RustlsConfig;
use rustls::ServerConfig;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio_rustls_acme::caches::DirCache;
use tokio_rustls_acme::AcmeConfig;
use tokio_stream::StreamExt;

/// Returns the certificate directory for a given hostname: `~/.ramekin/certs/{hostname}/`
fn cert_dir(hostname: &str) -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".ramekin").join("certs").join(hostname))
        .unwrap_or_else(|| PathBuf::from(".ramekin").join("certs").join(hostname))
}

/// Check if a valid certificate exists for the hostname.
/// Returns true if cert.pem exists and is valid for at least 7 more days.
fn is_cert_valid(hostname: &str) -> bool {
    let dir = cert_dir(hostname);
    let cert_path = dir.join("cert.pem");

    if !cert_path.exists() {
        return false;
    }

    // Try to parse the certificate and check expiration
    match std::fs::read(&cert_path) {
        Ok(pem_data) => check_cert_validity(&pem_data),
        Err(e) => {
            tracing::warn!("Failed to read certificate at {:?}: {}", cert_path, e);
            false
        }
    }
}

/// Parse PEM certificate data and check if it's valid for at least 7 more days.
fn check_cert_validity(pem_data: &[u8]) -> bool {
    use x509_parser::pem::parse_x509_pem;

    match parse_x509_pem(pem_data) {
        Ok((_, pem)) => match pem.parse_x509() {
            Ok(cert) => {
                let validity = cert.validity();
                match validity.time_to_expiration() {
                    Some(duration) => {
                        // time::Duration - convert to days for comparison
                        // 7 days = 7 * 24 * 60 * 60 seconds = 604800 seconds
                        let seven_days_secs = 7 * 24 * 60 * 60;
                        let duration_secs = duration.whole_seconds();
                        let is_valid = duration_secs > seven_days_secs;
                        if is_valid {
                            tracing::debug!(
                                "Certificate valid for {} more days",
                                duration_secs / 86400
                            );
                        } else {
                            tracing::info!(
                                "Certificate expires in {} days, will renew",
                                duration_secs / 86400
                            );
                        }
                        is_valid
                    }
                    None => {
                        tracing::warn!("Certificate has already expired");
                        false
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Failed to parse X.509 certificate: {}", e);
                false
            }
        },
        Err(e) => {
            tracing::warn!("Failed to parse PEM certificate: {}", e);
            false
        }
    }
}

/// Serve the application with ACME-obtained TLS certificate.
///
/// If a valid certificate exists, it will be used directly.
/// Otherwise, a new certificate will be obtained via ACME.
pub async fn serve_with_acme(
    app: Router,
    port: u16,
    hostname: &str,
    email: Option<&str>,
    staging: bool,
) {
    let cert_path = cert_dir(hostname);

    // Ensure the certificate directory exists
    if let Err(e) = std::fs::create_dir_all(&cert_path) {
        panic!(
            "Failed to create certificate directory {:?}: {}",
            cert_path, e
        );
    }

    // Check if we have a valid existing certificate
    if is_cert_valid(hostname) {
        tracing::info!("Using existing valid certificate for {}", hostname);
        serve_with_existing_cert(app, port, hostname).await;
        return;
    }

    tracing::info!("Obtaining certificate via ACME for {}", hostname);

    // Build the ACME configuration
    // Need to get the cert_dir again since we'll pass ownership to DirCache
    let mut config = AcmeConfig::new([hostname]).cache(DirCache::new(cert_dir(hostname)));

    // Add contact email if provided
    if let Some(email) = email {
        config = config.contact([format!("mailto:{}", email)]);
    }

    // Use staging or production Let's Encrypt directory
    // directory_lets_encrypt(true) = production, false = staging
    config = config.directory_lets_encrypt(!staging);

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
                Some(Ok(event)) => tracing::debug!("ACME event: {:?}", event),
                Some(Err(err)) => tracing::error!("ACME error: {:?}", err),
                None => break,
            }
        }
    });

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Server listening on https://{}:{}", hostname, port);

    axum_server::bind(addr)
        .acceptor(acceptor)
        .serve(app.into_make_service())
        .await
        .expect("Failed to start HTTPS server");
}

/// Serve the application using an existing certificate from disk.
async fn serve_with_existing_cert(app: Router, port: u16, hostname: &str) {
    let dir = cert_dir(hostname);
    let cert_path = dir.join("cert.pem");
    let key_path = dir.join("key.pem");

    let config = RustlsConfig::from_pem_file(&cert_path, &key_path)
        .await
        .unwrap_or_else(|e| panic!("Failed to load certificate from {:?}: {}", dir, e));

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Server listening on https://{}:{}", hostname, port);

    axum_server::bind_rustls(addr, config)
        .serve(app.into_make_service())
        .await
        .expect("Failed to start HTTPS server");
}
