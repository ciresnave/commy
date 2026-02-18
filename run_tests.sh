#!/bin/bash
# Commy Server Testing Guide
# This script helps you test the Commy WebSocket server locally

set -e  # Exit on error

echo "═════════════════════════════════════════════════════════════"
echo "Commy Server - Testing Setup"
echo "═════════════════════════════════════════════════════════════"

# Create test certificates
echo ""
echo "[1/4] Generating self-signed certificates for testing..."
if [ ! -f "dev-cert.pem" ] || [ ! -f "dev-key.pem" ]; then
    openssl req -x509 -newkey rsa:4096 \
        -keyout dev-key.pem \
        -out dev-cert.pem \
        -days 365 \
        -nodes \
        -subj "/CN=localhost" \
        -out dev-cert.pem
    echo "✓ Certificates generated (dev-cert.pem, dev-key.pem)"
else
    echo "✓ Certificates already exist"
fi

# Build the project
echo ""
echo "[2/4] Building project..."
cargo build
echo "✓ Build complete"

# Display startup instructions
echo ""
echo "[3/4] Ready to start server"
echo ""
echo "To run the server, execute in a terminal:"
echo ""
echo "  \$env:COMMY_TLS_CERT_PATH = \".\$(Get-Location | Split-Path -Leaf)\dev-cert.pem\""
echo "  \$env:COMMY_TLS_KEY_PATH = \".\$(Get-Location | Split-Path -Leaf)\dev-key.pem\""
echo "  \$env:COMMY_LISTEN_ADDR = \"127.0.0.1\""
echo "  \$env:COMMY_LISTEN_PORT = \"8443\""
echo "  \$env:COMMY_SERVER_ID = \"test-server\""
echo "  \$env:COMMY_CLUSTER_ENABLED = \"false\""
echo ""
echo "  cargo run --bin commy"
echo ""

echo "[4/4] Testing instructions"
echo ""
echo "In another terminal, install and use a WebSocket client:"
echo ""
echo "  npm install -g wscat"
echo "  wscat -c wss://127.0.0.1:8443 --no-check"
echo ""
echo "Once connected, you can send messages like:"
echo ""
echo '  {"type":"Heartbeat"}'
echo '  {"type":"Authenticate","payload":{"tenant_name":"test","method":"jwt","credentials":"test"}}'
echo ""
echo "═════════════════════════════════════════════════════════════"
echo "Setup complete! Follow the instructions above to test."
echo "═════════════════════════════════════════════════════════════"
