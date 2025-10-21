# Python ML Training Environment Setup Script

Write-Host "=====================================" -ForegroundColor Cyan
Write-Host "   ML Training Environment Setup     " -ForegroundColor Cyan
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host ""

# Check Python
Write-Host "Checking Python installation..." -ForegroundColor Blue
try {
    $pythonVersion = python --version
    Write-Host "✅ $pythonVersion" -ForegroundColor Green
} catch {
    Write-Host "❌ Python not found! Please install Python 3.9+" -ForegroundColor Red
    exit 1
}

# Create virtual environment
Write-Host "`nCreating virtual environment..." -ForegroundColor Blue
if (Test-Path "venv") {
    Write-Host "   venv already exists, removing..." -ForegroundColor Yellow
    Remove-Item -Recurse -Force venv
}

python -m venv venv
Write-Host "✅ Virtual environment created" -ForegroundColor Green

# Activate venv
Write-Host "`nActivating virtual environment..." -ForegroundColor Blue
.\venv\Scripts\Activate.ps1
Write-Host "✅ Virtual environment activated" -ForegroundColor Green

# Upgrade pip
Write-Host "`nUpgrading pip..." -ForegroundColor Blue
python -m pip install --upgrade pip | Out-Null
Write-Host "✅ pip upgraded" -ForegroundColor Green

# Install dependencies
Write-Host "`nInstalling dependencies..." -ForegroundColor Blue
Write-Host "   This may take 5-10 minutes..." -ForegroundColor Gray
pip install -r requirements.txt

if ($LASTEXITCODE -eq 0) {
    Write-Host "✅ All dependencies installed" -ForegroundColor Green
} else {
    Write-Host "❌ Dependency installation failed" -ForegroundColor Red
    exit 1
}

# Verify installation
Write-Host "`nVerifying installation..." -ForegroundColor Blue
$packages = @("torch", "scikit-learn", "xgboost", "onnx", "onnxruntime")

foreach ($pkg in $packages) {
    try {
        python -c "import $($pkg.Replace('-', '_')); print('  ✅ $pkg')"
    } catch {
        Write-Host "  ❌ $pkg not found" -ForegroundColor Red
    }
}

Write-Host ""
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host "   Setup Complete!                   " -ForegroundColor Cyan
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "To activate the environment:" -ForegroundColor Yellow
Write-Host "  .\venv\Scripts\Activate.ps1" -ForegroundColor White
Write-Host ""
Write-Host "To train models:" -ForegroundColor Yellow
Write-Host "  python scripts\train_trading_model.py" -ForegroundColor White
Write-Host "  python scripts\train_xgboost_model.py" -ForegroundColor White
Write-Host ""

