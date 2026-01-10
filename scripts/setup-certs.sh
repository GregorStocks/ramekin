#!/bin/bash
# Generate mkcert certificates for local HTTPS development
# Certs are stored in ~/.ramekin/certs/{hostname}/
# Uses UI_HOSTNAME env var, defaults to localhost

set -e

CERT_BASE="$HOME/.ramekin/certs"
HOSTNAME="${UI_HOSTNAME:-localhost}"
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
