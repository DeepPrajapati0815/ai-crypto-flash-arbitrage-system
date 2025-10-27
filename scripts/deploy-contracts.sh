#!/bin/bash
# Smart Contract Deployment Script for AI Crypto Flash Arbitrage System
# Production-ready deployment with comprehensive validation and safety checks

set -e  # Exit on any error

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

print_header() {
    echo -e "${CYAN}$1${NC}"
}

# Default values
NETWORK="sepolia"
VERIFY_CONTRACT=true
RUN_TESTS=true
SKIP_CONFIRMATION=false

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --network)
            NETWORK="$2"
            shift 2
            ;;
        --no-verify)
            VERIFY_CONTRACT=false
            shift
            ;;
        --no-tests)
            RUN_TESTS=false
            shift
            ;;
        --yes)
            SKIP_CONFIRMATION=true
            shift
            ;;
        --help)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --network NETWORK    Target network (sepolia, mainnet, polygon, arbitrum, optimism)"
            echo "  --no-verify          Skip contract verification"
            echo "  --no-tests          Skip contract testing"
            echo "  --yes               Skip confirmation prompts"
            echo "  --help              Show this help message"
            echo ""
            echo "Examples:"
            echo "  $0 --network sepolia"
            echo "  $0 --network mainnet --no-tests"
            echo "  $0 --network polygon --no-verify"
            exit 0
            ;;
        *)
            print_error "Unknown option: $1"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

print_header "============================================================"
print_header "AI CRYPTO FLASH ARBITRAGE SYSTEM - CONTRACT DEPLOYMENT"
print_header "============================================================"
print_header "Network: $NETWORK"
print_header "Verify Contract: $VERIFY_CONTRACT"
print_header "Run Tests: $RUN_TESTS"
print_header "============================================================"

# Check if Node.js and npm are installed
print_status "Checking Node.js installation..."
if ! command -v node &> /dev/null; then
    print_error "Node.js is not installed. Please install Node.js first."
    echo "Visit: https://nodejs.org/"
    exit 1
fi

if ! command -v npm &> /dev/null; then
    print_error "npm is not installed. Please install npm first."
    exit 1
fi

NODE_VERSION=$(node --version)
NPM_VERSION=$(npm --version)
print_success "Node.js: $NODE_VERSION"
print_success "npm: $NPM_VERSION"

# Check if .env file exists
print_status "Checking environment configuration..."
if [ ! -f .env ]; then
    print_warning ".env file not found. Creating from template..."
    if [ -f env.example ]; then
        cp env.example .env
        print_success ".env file created from template"
        print_warning "Please edit .env file with your actual values before continuing"
        echo ""
        echo "Required environment variables:"
        echo "  - EVM_PRIVATE_KEY (your wallet private key without 0x prefix)"
        echo "  - EVM_RPC_URL (Ethereum RPC endpoint)"
        echo "  - ETHERSCAN_API_KEY (for contract verification)"
        echo ""
        
        if [ "$SKIP_CONFIRMATION" = false ]; then
            read -p "Press Enter after editing .env file, or Ctrl+C to exit..."
        fi
    else
        print_error "env.example file not found!"
        exit 1
    fi
else
    print_success ".env file found"
fi

# Load environment variables
source .env

# Validate required environment variables
print_status "Validating environment variables..."

required_vars=(
    "EVM_PRIVATE_KEY"
    "EVM_RPC_URL"
)

missing_vars=()
for var in "${required_vars[@]}"; do
    if [[ -z "${!var}" ]]; then
        missing_vars+=("$var")
    fi
done

if [[ ${#missing_vars[@]} -gt 0 ]]; then
    print_error "Missing required environment variables:"
    for var in "${missing_vars[@]}"; do
        echo "  - $var"
    done
    echo ""
    echo "Please set these variables in your .env file"
    exit 1
fi

# Validate private key format
if [[ ! "$EVM_PRIVATE_KEY" =~ ^[0-9a-fA-F]{64}$ ]]; then
    print_error "Invalid private key format. Should be 64 hex characters without 0x prefix"
    exit 1
fi

print_success "Environment variables validated"

# Install dependencies if needed
print_status "Checking dependencies..."
if [ ! -d "node_modules" ]; then
    print_status "Installing Node.js dependencies..."
    npm install
    print_success "Dependencies installed"
else
    print_success "Dependencies already installed"
fi

# Compile contracts
print_status "Compiling smart contracts..."
npx hardhat compile
print_success "Contracts compiled successfully"

# Check wallet balance
print_status "Checking wallet balance..."
WALLET_ADDRESS=$(npx hardhat run --network $NETWORK -e "console.log((await ethers.getSigners())[0].address)" 2>/dev/null | tail -1)
BALANCE_WEI=$(npx hardhat run --network $NETWORK -e "console.log((await ethers.provider.getBalance((await ethers.getSigners())[0].address)).toString())" 2>/dev/null | tail -1)
BALANCE_ETH=$(echo "scale=4; $BALANCE_WEI / 1000000000000000000" | bc)

print_status "Wallet Address: $WALLET_ADDRESS"
print_status "Balance: $BALANCE_ETH ETH"

# Check minimum balance
MIN_BALANCE=0.1
if (( $(echo "$BALANCE_ETH < $MIN_BALANCE" | bc -l) )); then
    print_error "Insufficient balance. Need at least $MIN_BALANCE ETH for deployment"
    print_error "Current balance: $BALANCE_ETH ETH"
    exit 1
fi

print_success "Wallet balance sufficient"

# Network-specific validation
print_status "Validating network configuration..."
case $NETWORK in
    mainnet)
        print_warning "Deploying to MAINNET - This will cost real ETH!"
        if [ "$SKIP_CONFIRMATION" = false ]; then
            read -p "Are you sure you want to deploy to mainnet? (yes/no): " confirm
            if [[ $confirm != "yes" ]]; then
                print_error "Deployment cancelled"
                exit 1
            fi
        fi
        ;;
    sepolia|goerli)
        print_status "Deploying to testnet: $NETWORK"
        ;;
    polygon|arbitrum|optimism)
        print_status "Deploying to L2: $NETWORK"
        ;;
    *)
        print_error "Unsupported network: $NETWORK"
        echo "Supported networks: sepolia, mainnet, polygon, arbitrum, optimism"
        exit 1
        ;;
esac

# Deploy contract
print_header "============================================================"
print_header "DEPLOYING CONTRACT"
print_header "============================================================"

print_status "Deploying FlashArbProductionSafe to $NETWORK..."
DEPLOY_OUTPUT=$(npx hardhat run scripts/deploy.js --network $NETWORK 2>&1)

if [ $? -eq 0 ]; then
    print_success "Contract deployed successfully!"
    
    # Extract contract address from output
    CONTRACT_ADDRESS=$(echo "$DEPLOY_OUTPUT" | grep "Contract Address:" | cut -d' ' -f3)
    TRANSACTION_HASH=$(echo "$DEPLOY_OUTPUT" | grep "Transaction hash:" | cut -d' ' -f3)
    
    print_success "Contract Address: $CONTRACT_ADDRESS"
    print_success "Transaction Hash: $TRANSACTION_HASH"
else
    print_error "Contract deployment failed!"
    echo "$DEPLOY_OUTPUT"
    exit 1
fi

# Verify contract if requested
if [ "$VERIFY_CONTRACT" = true ]; then
    print_header "============================================================"
    print_header "VERIFYING CONTRACT"
    print_header "============================================================"
    
    print_status "Verifying contract on block explorer..."
    VERIFY_OUTPUT=$(npx hardhat run scripts/verify.js --network $NETWORK 2>&1)
    
    if [ $? -eq 0 ]; then
        print_success "Contract verified successfully!"
    else
        print_warning "Contract verification failed or is pending"
        echo "$VERIFY_OUTPUT"
    fi
fi

# Run tests if requested
if [ "$RUN_TESTS" = true ]; then
    print_header "============================================================"
    print_header "RUNNING CONTRACT TESTS"
    print_header "============================================================"
    
    print_status "Running contract validation tests..."
    TEST_OUTPUT=$(npx hardhat run scripts/test-contract.js --network $NETWORK 2>&1)
    
    if [ $? -eq 0 ]; then
        print_success "All contract tests passed!"
    else
        print_warning "Some contract tests failed"
        echo "$TEST_OUTPUT"
    fi
fi

# Update environment file with contract address
print_status "Updating environment configuration..."
if grep -q "FLASH_ARB_ADDRESS=" .env; then
    sed -i "s/FLASH_ARB_ADDRESS=.*/FLASH_ARB_ADDRESS=$CONTRACT_ADDRESS/" .env
else
    echo "FLASH_ARB_ADDRESS=$CONTRACT_ADDRESS" >> .env
fi

# Add network-specific address
NETWORK_KEY="FLASH_ARB_ADDRESS_$(echo $NETWORK | tr '[:lower:]' '[:upper:]')"
if grep -q "$NETWORK_KEY=" .env; then
    sed -i "s/$NETWORK_KEY=.*/$NETWORK_KEY=$CONTRACT_ADDRESS/" .env
else
    echo "$NETWORK_KEY=$CONTRACT_ADDRESS" >> .env
fi

print_success "Environment file updated"

# Generate deployment summary
print_header "============================================================"
print_header "DEPLOYMENT SUMMARY"
print_header "============================================================"

echo "Network: $NETWORK"
echo "Contract Address: $CONTRACT_ADDRESS"
echo "Transaction Hash: $TRANSACTION_HASH"
echo "Wallet Address: $WALLET_ADDRESS"
echo "Balance After: $(echo "scale=4; $(npx hardhat run --network $NETWORK -e "console.log((await ethers.provider.getBalance((await ethers.getSigners())[0].address)).toString())" 2>/dev/null | tail -1) / 1000000000000000000" | bc) ETH"

# Get block explorer URL
case $NETWORK in
    mainnet)
        EXPLORER_URL="https://etherscan.io/address/$CONTRACT_ADDRESS"
        ;;
    sepolia)
        EXPLORER_URL="https://sepolia.etherscan.io/address/$CONTRACT_ADDRESS"
        ;;
    goerli)
        EXPLORER_URL="https://goerli.etherscan.io/address/$CONTRACT_ADDRESS"
        ;;
    polygon)
        EXPLORER_URL="https://polygonscan.com/address/$CONTRACT_ADDRESS"
        ;;
    arbitrum)
        EXPLORER_URL="https://arbiscan.io/address/$CONTRACT_ADDRESS"
        ;;
    optimism)
        EXPLORER_URL="https://optimistic.etherscan.io/address/$CONTRACT_ADDRESS"
        ;;
esac

echo "Block Explorer: $EXPLORER_URL"

print_header "============================================================"
print_success "DEPLOYMENT COMPLETED SUCCESSFULLY!"
print_header "============================================================"

echo ""
echo "Next steps:"
echo "1. Verify the contract on the block explorer: $EXPLORER_URL"
echo "2. Test the contract functions using the Rust application"
echo "3. Monitor the contract for any issues"
echo "4. Update your Rust application configuration with the new contract address"
echo ""
echo "Contract address has been saved to .env file"
echo "Deployment information saved to deployments/$NETWORK.json"
echo ""
echo "Happy Trading! 🚀📈"
