@echo off
echo 🚀 Deploying Flash Arbitrage Contracts to Sepolia Testnet
echo.

echo 📋 Checking prerequisites...
if not exist .env (
    echo ❌ .env file not found!
    echo Please create .env file with your configuration
    echo See DEPLOYMENT_GUIDE.md for details
    pause
    exit /b 1
)

echo ✅ .env file found
echo.

echo 🔨 Compiling contracts...
npx hardhat compile
if %errorlevel% neq 0 (
    echo ❌ Compilation failed!
    pause
    exit /b 1
)

echo ✅ Compilation successful
echo.

echo 🚀 Deploying to Sepolia...
npx hardhat run scripts/deploy-sepolia.js --network sepolia
if %errorlevel% neq 0 (
    echo ❌ Deployment failed!
    pause
    exit /b 1
)

echo ✅ Deployment successful!
echo.

echo 🔍 Verifying contracts...
npx hardhat run scripts/verify-sepolia.js --network sepolia

echo.
echo 🎉 Deployment process completed!
echo Check deployments/sepolia-deployment.json for contract addresses
pause
