# PowerShell script to set up Visual Studio environment and run HFT Bot

Write-Host "Setting up Visual Studio environment..." -ForegroundColor Green

# Try to find Visual Studio Build Tools
$vsPaths = @(
    "C:\Program Files\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat",
    "C:\Program Files (x86)\Microsoft Visual Studio\2019\BuildTools\VC\Auxiliary\Build\vcvars64.bat",
    "C:\Program Files\Microsoft Visual Studio\2019\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
)

$found = $false
foreach ($path in $vsPaths) {
    if (Test-Path $path) {
        Write-Host "Found Visual Studio at: $path" -ForegroundColor Yellow
        Write-Host "Setting up environment..." -ForegroundColor Yellow
        
        # Set up the environment
        cmd /c "`"$path`" && set" | ForEach-Object {
            if ($_ -match '^([^=]+)=(.*)$') {
                [Environment]::SetEnvironmentVariable($matches[1], $matches[2])
            }
        }
        $found = $true
        break
    }
}

if (-not $found) {
    Write-Host "Visual Studio Build Tools not found!" -ForegroundColor Red
    Write-Host "Please install Visual Studio Build Tools 2022 with C++ workload" -ForegroundColor Red
    Write-Host "Download from: https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022" -ForegroundColor Yellow
    exit 1
}

Write-Host "Setting up Rust environment..." -ForegroundColor Green
$env:PATH += ";C:\Program Files\Rust stable MSVC 1.90\bin"

Write-Host "Running HFT Bot..." -ForegroundColor Green
& "C:\Program Files\Rust stable MSVC 1.90\bin\cargo.exe" run

