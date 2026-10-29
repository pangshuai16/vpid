#!/usr/bin/env bash
set -e
export PATH=/usr/local/cargo/bin:$PATH
export CARGO_TARGET_DIR=/build/target

echo "== deps =="
pkg-config --modversion gtk+-3.0 || { echo "missing gtk, run linux_setup_deps.sh"; exit 1; }

echo "== sync source (preserve /build/target) =="
rm -rf /build/rust && mkdir -p /build
cd /vpid/rust
tar cf - --exclude=target . | (cd /build && mkdir -p rust && cd rust && tar xf -)
echo "synced $(du -sh /build/rust | cut -f1)"

echo "== cargo build -p vpid-ui-linux =="
cd /build/rust
cargo build -p vpid-ui-linux 2>&1 | tail -40

echo "== cargo test -p vpid-core -p vpid-ui-linux =="
cargo test -p vpid-core -p vpid-ui-linux 2>&1 | tail -40

echo "== xvfb demo smoke =="
set +e
timeout 5 xvfb-run -a /build/target/debug/vpid-viewer-linux --demo 2>&1 | head -20
echo "demo_exit=${PIPESTATUS[0]}"
timeout 5 xvfb-run -a /build/target/debug/vpid-viewer-linux --scan 2>&1 | head -20
echo "scan_exit=${PIPESTATUS[0]}"
set -e
echo "== BUILD OK =="