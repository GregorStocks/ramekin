#!/bin/bash
# Generate mkcert certificates for local HTTPS development
# Certs are stored in ~/.ramekin/certs/{hostname}/
# Uses RAMEKIN_SELF_SIGNED_URL env var, defaults to https://localhost:5173

set -e

CERT_BASE="$HOME/.ramekin/certs"
SELF_SIGNED_URL="${RAMEKIN_SELF_SIGNED_URL:-https://localhost:5173}"
# Strip scheme, port, and path to get bare hostname
HOSTNAME="$(echo "$SELF_SIGNED_URL" | sed -E 's#^[a-z]+://([^:/]+).*#\1#')"
CERT_DIR="$CERT_BASE/$HOSTNAME"

# Check if certs already exist
if [ -f "$CERT_DIR/cert.pem" ] && [ -f "$CERT_DIR/key.pem" ]; then
    echo "Certs already exist for $HOSTNAME"
    exit 0
fi

# Check if mkcert is installed
if ! command -v mkcert &> /dev/null; then
    echo "Error: mkcert is not installed"
    echo "Install with: brew install mkcert && mkcert -install"
    exit 1
fi

# Ensure mkcert CA is installed (check if rootCA.pem exists)
CAROOT="$(mkcert -CAROOT)"
if [ ! -f "$CAROOT/rootCA.pem" ]; then
    echo "Installing mkcert CA (may require sudo)..."
    mkcert -install
fi

echo "Generating certs for $HOSTNAME..."
mkdir -p "$CERT_DIR"
mkcert -cert-file "$CERT_DIR/cert.pem" -key-file "$CERT_DIR/key.pem" "$HOSTNAME"
echo "Done! Certs are in $CERT_DIR/"
