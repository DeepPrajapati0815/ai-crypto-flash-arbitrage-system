#!/bin/bash
# =============================================================================
# Fix Rust Dependencies Script
# =============================================================================
# This script fixes edition2024 dependency issues by updating Cargo.lock

set -e

echo "🔧 Fixing Rust dependencies..."
echo "=============================="

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    echo "❌ Rust/Cargo is not installed"
    echo "   Install from: https://rustup.rs/"
    exit 1
fi

echo "✅ Rust found: $(rustc --version)"

# Backup existing Cargo.lock
if [ -f Cargo.lock ]; then
    echo "📦 Backing up Cargo.lock..."
    cp Cargo.lock Cargo.lock.backup
    echo "✅ Backup created: Cargo.lock.backup"
fi

# Option 1: Update all dependencies
echo ""
echo "Updating dependencies to compatible versions..."
cargo update

# Option 2: Try building with nightly (if available)
if cargo +nightly --version &> /dev/null; then
    echo "✅ Rust nightly available"
    echo "   Building with nightly to test..."
    cargo +nightly build --release
else
    echo "⚠️  Rust nightly not installed"
    echo "   Install with: rustup install nightly"
    echo "   Trying stable build..."
    cargo build --release
fi

echo ""
echo "=============================="
echo "✅ Dependencies fixed!"
echo ""
echo "To build Docker image now:"
echo "  docker compose -f docker-compose.local.yml up -d --build"

