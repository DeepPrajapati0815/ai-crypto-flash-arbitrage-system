# Smart Contract Deployment Script for AI Crypto Flash Arbitrage System
# PowerShell version for Windows users
# Production-ready deployment with comprehensive validation and safety checks

param(
    [string]$Network = "sepolia",
    [switch]$NoVerify,
    [switch]$NoTests,
    [switch]$Yes,
    [switch]$Help
)

# Show help if requested
if ($Help) {
    Write-Host "Usage: .\deploy-contracts.ps1 [OPTIONS]" -ForegroundColor Cyan
    Write-Host ""
    Write-Host "Options:" -ForegroundColor Cyan
    Write-Host "  -Network NETWORK    Target network (sepolia, mainnet, polygon, arbitrum, optimism)" -ForegroundColor White
    Write-Host "  -NoVerify          Skip contract verification" -ForegroundColor White
    Write-Host "  -NoTests          Skip contract testing" -ForegroundColor White
    Write-Host "  -Yes              Skip confirmation prompts" -ForegroundColor White
    Write-Host "  -Help             Show this help message" -ForegroundColor White
    Write-Host ""
    Write-Host "Examples:" -ForegroundColor Cyan
    Write-Host "  .\deploy-contracts.ps1 -Network sepolia" -ForegroundColor White
    Write-Host "  .\deploy-contracts.ps1 -Network mainnet -NoTests" -ForegroundColor White
    Write-Host "  .\deploy-contracts.ps1 -Network polygon -NoVerify" -ForegroundColor White
    exit 0
}

# Set error action preference
$ErrorActionPreference = "Stop"

# Function to print colored output
function Write-Status {
    param([string]$Message)
    Write-Host "[INFO] $Message" -ForegroundColor Blue
}

function Write-Success {
    param([string]$Message)
    Write-Host "[SUCCESS] $Message" -ForegroundColor Green
}

function Write-Warning {
    param([string]$Message)
    Write-Host "[WARNING] $Message" -ForegroundColor Yellow
}

function Write-Error {
    param([string]$Message)
    Write-Host "[ERROR] $Message" -ForegroundColor Red
}

function Write-Header {
    param([string]$Message)
    Write-Host $Message -ForegroundColor Cyan
}

# Set default values
$VerifyContract = -not $NoVerify
$RunTests = -not $NoTests
$SkipConfirmation = $Yes

Write-Header "============================================================"
Write-Header "AI CRYPTO FLASH ARBITRAGE SYSTEM - CONTRACT DEPLOYMENT"
Write-Header "============================================================"
Write-Header "Network: $Network"
Write-Header "Verify Contract: $VerifyContract"
Write-Header "Run Tests: $RunTests"
Write-Header "============================================================"

# Check if Node.js and npm are installed
Write-Status "Checking Node.js installation..."
try {
    $nodeVersion = node --version
    $npmVersion = npm --version
    Write-Success "Node.js: $nodeVersion"
    Write-Success "npm: $npmVersion"
} catch {
    Write-Error "Node.js is not installed. Please install Node.js first."
    Write-Host "Visit: https://nodejs.org/" -ForegroundColor Yellow
    exit 1
}

# Check if .env file exists
Write-Status "Checking environment configuration..."
if (-not (Test-Path .env)) {
    Write-Warning ".env file not found. Creating from template..."
    if (Test-Path env.example) {
        Copy-Item env.example .env
        Write-Success ".env file created from template"
        Write-Warning "Please edit .env file with your actual values before continuing"
        Write-Host ""
        Write-Host "Required environment variables:" -ForegroundColor Yellow
        Write-Host "  - EVM_PRIVATE_KEY (your wallet private key without 0x prefix)" -ForegroundColor White
        Write-Host "  - EVM_RPC_URL (Ethereum RPC endpoint)" -ForegroundColor White
        Write-Host "  - ETHERSCAN_API_KEY (for contract verification)" -ForegroundColor White
        Write-Host ""
        
        if (-not $SkipConfirmation) {
            Write-Host "Press Enter after editing .env file, or Ctrl+C to exit..." -ForegroundColor Yellow
            Read-Host
        }
    } else {
        Write-Error "env.example file not found!"
        exit 1
    }
} else {
    Write-Success ".env file found"
}

# Load environment variables
Write-Status "Loading environment variables..."
Get-Content .env | ForEach-Object {
    if ($_ -match "^([^=]+)=(.*)$") {
        $name = $matches[1]
        $value = $matches[2]
        [Environment]::SetEnvironmentVariable($name, $value, "Process")
    }
}

# Validate required environment variables
Write-Status "Validating environment variables..."

$requiredVars = @("EVM_PRIVATE_KEY", "EVM_RPC_URL")
$missingVars = @()

foreach ($var in $requiredVars) {
    if (-not [Environment]::GetEnvironmentVariable($var, "Process")) {
        $missingVars += $var
    }
}

if ($missingVars.Count -gt 0) {
    Write-Error "Missing required environment variables:"
    foreach ($var in $missingVars) {
        Write-Host "  - $var" -ForegroundColor Red
    }
    Write-Host ""
    Write-Host "Please set these variables in your .env file" -ForegroundColor Yellow
    exit 1
}

# Validate private key format
$privateKey = [Environment]::GetEnvironmentVariable("EVM_PRIVATE_KEY", "Process")
if ($privateKey -notmatch "^[0-9a-fA-F]{64}$") {
    Write-Error "Invalid private key format. Should be 64 hex characters without 0x prefix"
    exit 1
}

Write-Success "Environment variables validated"

# Install dependencies if needed
Write-Status "Checking dependencies..."
if (-not (Test-Path node_modules)) {
    Write-Status "Installing Node.js dependencies..."
    npm install
    Write-Success "Dependencies installed"
} else {
    Write-Success "Dependencies already installed"
}

# Compile contracts
Write-Status "Compiling smart contracts..."
npx hardhat compile
Write-Success "Contracts compiled successfully"

# Check wallet balance
Write-Status "Checking wallet balance..."
$walletAddress = npx hardhat run --network $Network -e "console.log((await ethers.getSigners())[0].address)" 2>$null | Select-Object -Last 1
$balanceWei = npx hardhat run --network $Network -e "console.log((await ethers.provider.getBalance((await ethers.getSigners())[0].address)).toString())" 2>$null | Select-Object -Last 1
$balanceEth = [math]::Round([decimal]$balanceWei / 1000000000000000000, 4)

Write-Status "Wallet Address: $walletAddress"
Write-Status "Balance: $balanceEth ETH"

# Check minimum balance
$minBalance = 0.1
if ([decimal]$balanceEth -lt [decimal]$minBalance) {
    Write-Error "Insufficient balance. Need at least $minBalance ETH for deployment"
    Write-Error "Current balance: $balanceEth ETH"
    exit 1
}

Write-Success "Wallet balance sufficient"

# Network-specific validation
Write-Status "Validating network configuration..."
switch ($Network) {
    "mainnet" {
        Write-Warning "Deploying to MAINNET - This will cost real ETH!"
        if (-not $SkipConfirmation) {
            $confirm = Read-Host "Are you sure you want to deploy to mainnet? (yes/no)"
            if ($confirm -ne "yes") {
                Write-Error "Deployment cancelled"
                exit 1
            }
        }
    }
    { $_ -in @("sepolia", "goerli") } {
        Write-Status "Deploying to testnet: $Network"
    }
    { $_ -in @("polygon", "arbitrum", "optimism") } {
        Write-Status "Deploying to L2: $Network"
    }
    default {
        Write-Error "Unsupported network: $Network"
        Write-Host "Supported networks: sepolia, mainnet, polygon, arbitrum, optimism" -ForegroundColor Yellow
        exit 1
    }
}

# Deploy contract
Write-Header "============================================================"
Write-Header "DEPLOYING CONTRACT"
Write-Header "============================================================"

Write-Status "Deploying FlashArbProductionSafe to $Network..."
try {
    $deployOutput = npx hardhat run scripts/deploy.js --network $Network 2>&1
    Write-Success "Contract deployed successfully!"
    
    # Extract contract address from output
    $contractAddress = ($deployOutput | Select-String "Contract Address:").Line -replace ".*Contract Address: ", ""
    $transactionHash = ($deployOutput | Select-String "Transaction hash:").Line -replace ".*Transaction hash: ", ""
    
    Write-Success "Contract Address: $contractAddress"
    Write-Success "Transaction Hash: $transactionHash"
} catch {
    Write-Error "Contract deployment failed!"
    Write-Host $_.Exception.Message -ForegroundColor Red
    exit 1
}

# Verify contract if requested
if ($VerifyContract) {
    Write-Header "============================================================"
    Write-Header "VERIFYING CONTRACT"
    Write-Header "============================================================"
    
    Write-Status "Verifying contract on block explorer..."
    try {
        $verifyOutput = npx hardhat run scripts/verify.js --network $Network 2>&1
        Write-Success "Contract verified successfully!"
    } catch {
        Write-Warning "Contract verification failed or is pending"
        Write-Host $_.Exception.Message -ForegroundColor Yellow
    }
}

# Run tests if requested
if ($RunTests) {
    Write-Header "============================================================"
    Write-Header "RUNNING CONTRACT TESTS"
    Write-Header "============================================================"
    
    Write-Status "Running contract validation tests..."
    try {
        $testOutput = npx hardhat run scripts/test-contract.js --network $Network 2>&1
        Write-Success "All contract tests passed!"
    } catch {
        Write-Warning "Some contract tests failed"
        Write-Host $_.Exception.Message -ForegroundColor Yellow
    }
}

# Update environment file with contract address
Write-Status "Updating environment configuration..."
$envContent = Get-Content .env

# Update main contract address
$envContent = $envContent -replace "^FLASH_ARB_ADDRESS=.*", "FLASH_ARB_ADDRESS=$contractAddress"

# Add network-specific address
$networkKey = "FLASH_ARB_ADDRESS_$($Network.ToUpper())"
$envContent = $envContent -replace "^$networkKey=.*", "$networkKey=$contractAddress"

# Add if not exists
if ($envContent -notmatch "^FLASH_ARB_ADDRESS=") {
    $envContent += "`nFLASH_ARB_ADDRESS=$contractAddress"
}
if ($envContent -notmatch "^$networkKey=") {
    $envContent += "`n$networkKey=$contractAddress"
}

Set-Content .env $envContent
Write-Success "Environment file updated"

# Generate deployment summary
Write-Header "============================================================"
Write-Header "DEPLOYMENT SUMMARY"
Write-Header "============================================================"

Write-Host "Network: $Network"
Write-Host "Contract Address: $contractAddress"
Write-Host "Transaction Hash: $transactionHash"
Write-Host "Wallet Address: $walletAddress"

# Get balance after deployment
$balanceAfterWei = npx hardhat run --network $Network -e "console.log((await ethers.provider.getBalance((await ethers.getSigners())[0].address)).toString())" 2>$null | Select-Object -Last 1
$balanceAfterEth = [math]::Round([decimal]$balanceAfterWei / 1000000000000000000, 4)
Write-Host "Balance After: $balanceAfterEth ETH"

# Get block explorer URL
$explorerUrl = switch ($Network) {
    "mainnet" { "https://etherscan.io/address/$contractAddress" }
    "sepolia" { "https://sepolia.etherscan.io/address/$contractAddress" }
    "goerli" { "https://goerli.etherscan.io/address/$contractAddress" }
    "polygon" { "https://polygonscan.com/address/$contractAddress" }
    "arbitrum" { "https://arbiscan.io/address/$contractAddress" }
    "optimism" { "https://optimistic.etherscan.io/address/$contractAddress" }
}

Write-Host "Block Explorer: $explorerUrl"

Write-Header "============================================================"
Write-Success "DEPLOYMENT COMPLETED SUCCESSFULLY!"
Write-Header "============================================================"

Write-Host ""
Write-Host "Next steps:" -ForegroundColor Cyan
Write-Host "1. Verify the contract on the block explorer: $explorerUrl" -ForegroundColor White
Write-Host "2. Test the contract functions using the Rust application" -ForegroundColor White
Write-Host "3. Monitor the contract for any issues" -ForegroundColor White
Write-Host "4. Update your Rust application configuration with the new contract address" -ForegroundColor White
Write-Host ""
Write-Host "Contract address has been saved to .env file" -ForegroundColor Green
Write-Host "Deployment information saved to deployments/$Network.json" -ForegroundColor Green
Write-Host ""
Write-Host "Happy Trading! 🚀📈" -ForegroundColor Green
