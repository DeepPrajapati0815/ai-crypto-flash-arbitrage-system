# PowerShell script to run the HFT Bot
Write-Host "Setting up Rust environment..." -ForegroundColor Green
$env:PATH += ";C:\Program Files\Rust stable GNU 1.90\bin"

Write-Host "Compiling HFT Bot..." -ForegroundColor Yellow
& "C:\Program Files\Rust stable GNU 1.90\bin\cargo.exe" build --release

if ($LASTEXITCODE -eq 0) {
    Write-Host "Compilation successful! Running HFT Bot..." -ForegroundColor Green
    & "C:\Program Files\Rust stable GNU 1.90\bin\cargo.exe" run --release
} else {
    Write-Host "Compilation failed!" -ForegroundColor Red
}

