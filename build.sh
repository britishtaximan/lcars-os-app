#!/bin/bash
# LCARS OS — Build Script
# Builds shared metrics helper, then Electron and Tauri versions
# Run from the lcars-os-app root directory: ./build.sh

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ELECTRON_DIR="$SCRIPT_DIR/lcars-os-electron"
TAURI_DIR="$SCRIPT_DIR/lcars-os-tauri"
METRICS_DIR="$SCRIPT_DIR/lcars-metrics"

echo "============================================"
echo "  LCARS OS — Building All Components"
echo "============================================"
echo ""

# Check prerequisites
echo ">> Checking prerequisites..."

if ! command -v node &> /dev/null; then
    echo "ERROR: Node.js is not installed. Install it from https://nodejs.org"
    exit 1
fi

if ! command -v npm &> /dev/null; then
    echo "ERROR: npm is not installed."
    exit 1
fi

if ! command -v cargo &> /dev/null; then
    echo "WARNING: Rust/Cargo is not installed. Rust builds will be skipped."
    echo "  Install Rust from https://rustup.rs"
    SKIP_RUST=1
fi

echo "  Node.js: $(node --version)"
echo "  npm: $(npm --version)"
if [ -z "$SKIP_RUST" ]; then
    echo "  Rust: $(rustc --version)"
    echo "  Cargo: $(cargo --version)"
fi
echo ""

# Verify shared files
if [ ! -f "$SCRIPT_DIR/frontend/index.html" ]; then
    echo "ERROR: Shared frontend/index.html not found in $SCRIPT_DIR"
    exit 1
fi
echo ">> Shared frontend/index.html found ($(wc -c < "$SCRIPT_DIR/frontend/index.html") bytes)"
echo ""

# ---- Build Shared Metrics Binary (needed by Electron) ----
build_metrics() {
    if [ -n "$SKIP_RUST" ]; then
        echo ">> Skipping lcars-metrics build (Rust not installed)"
        return 1
    fi

    echo "============================================"
    echo "  Building Shared Metrics Helper..."
    echo "============================================"
    cd "$METRICS_DIR"

    echo ">> Compiling lcars-metrics (Rust)..."
    cargo build --release
    echo ">> lcars-metrics build complete!"
    echo "   Binary: $METRICS_DIR/target/release/lcars-metrics"
    echo ""
}

# ---- Build Electron ----
build_electron() {
    echo "============================================"
    echo "  Building Electron Version..."
    echo "============================================"
    cd "$ELECTRON_DIR"

    if [ ! -d "node_modules" ]; then
        echo ">> Installing Electron dependencies..."
        npm install
    else
        echo ">> node_modules exists, skipping install (run 'npm install' to update)"
    fi

    echo ">> Building Electron app..."
    npx electron-builder --mac
    echo ""
    echo ">> Electron build complete!"
    echo "   Output: $ELECTRON_DIR/dist/"
    echo ""
}

# ---- Build Tauri ----
build_tauri() {
    if [ -n "$SKIP_RUST" ]; then
        echo "============================================"
        echo "  Skipping Tauri Build (Rust not installed)"
        echo "============================================"
        echo ""
        return
    fi

    echo "============================================"
    echo "  Building Tauri Version..."
    echo "============================================"
    cd "$TAURI_DIR"

    echo ">> Building Tauri app (this may take a while on first run)..."
    cargo build --release
    echo ""
    echo ">> Tauri build complete!"
    echo "   Binary: $TAURI_DIR/target/release/lcars-os"
    echo ""
}

# ---- Run Builds ----
BUILD_MODE="${1:-all}"

case "$BUILD_MODE" in
    metrics)
        build_metrics
        ;;
    electron)
        build_metrics
        build_electron
        ;;
    tauri)
        build_tauri
        ;;
    all)
        # Build metrics first (Electron depends on it)
        echo ">> Step 1: Building shared metrics binary..."
        echo ""
        build_metrics

        # Then build Electron and Tauri in parallel
        echo ">> Step 2: Building Electron and Tauri in parallel..."
        echo ""
        build_electron &
        ELECTRON_PID=$!
        build_tauri &
        TAURI_PID=$!

        # Wait for both
        ELECTRON_STATUS=0
        TAURI_STATUS=0
        wait $ELECTRON_PID || ELECTRON_STATUS=$?
        wait $TAURI_PID || TAURI_STATUS=$?

        echo "============================================"
        echo "  Build Summary"
        echo "============================================"
        echo "  Metrics:  SUCCESS"
        if [ $ELECTRON_STATUS -eq 0 ]; then
            echo "  Electron: SUCCESS"
        else
            echo "  Electron: FAILED (exit code $ELECTRON_STATUS)"
        fi
        if [ $TAURI_STATUS -eq 0 ]; then
            echo "  Tauri:    SUCCESS"
        else
            echo "  Tauri:    FAILED (exit code $TAURI_STATUS)"
        fi
        echo "============================================"
        ;;
    *)
        echo "Usage: ./build.sh [metrics|electron|tauri|all]"
        echo "  metrics   - Build only the shared metrics helper"
        echo "  electron  - Build metrics + Electron version"
        echo "  tauri     - Build only the Tauri version"
        echo "  all       - Build everything (default)"
        exit 1
        ;;
esac

echo ""
echo "Done!"
