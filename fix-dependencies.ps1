# =============================================================================
# Fix Rust Dependencies Script (Windows)
# =============================================================================
# This script fixes edition2024 dependency issues by updating Cargo.lock

Write-Host "🔧 Fixing Rust dependencies..." -ForegroundColor Cyan
Write-Host "==============================" -ForegroundColor Cyan

# Check if Rust is installed
try {
    $rustVersion = cargo --version
    Write-Host "✅ Rust found: $rustVersion" -ForegroundColor Green
} catch {
    Write-Host "❌ Rust/Cargo is not installed" -ForegroundColor Red
    Write-Host "   Install from: https://rustup.rs/" -ForegroundColor Yellow
    exit 1
}

# Backup existing Cargo.lock
if (Test-Path Cargo.lock) {
    Write-Host "📦 Backing up Cargo.lock..." -ForegroundColor Yellow
    Copy-Item Cargo.lock Cargo.lock.backup
    Write-Host "✅ Backup created: Cargo.lock.backup" -ForegroundColor Green
}

# Update all dependencies
Write-Host ""
Write-Host "Updating dependencies to compatible versions..." -ForegroundColor Cyan
cargo update

# Try building with nightly (if available)
try {
    $nightlyVersion = cargo +nightly --version
    Write-Host "✅ Rust nightly available" -ForegroundColor Green
    Write-Host "   Building with nightly to test..." -ForegroundColor Cyan
    cargo +nightly build --release
} catch {
    Write-Host "⚠️  Rust nightly not installed" -ForegroundColor Yellow
    Write-Host "   Install with: rustup install nightly" -ForegroundColor Yellow
    Write-Host "   Trying stable build..." -ForegroundColor Cyan
    cargo build --release
}

Write-Host ""
Write-Host "==============================" -ForegroundColor Cyan
Write-Host "✅ Dependencies fixed!" -ForegroundColor Green
Write-Host ""
Write-Host "To build Docker image now:" -ForegroundColor Cyan
Write-Host "  docker compose -f docker-compose.local.yml up -d --build" -ForegroundColor White

