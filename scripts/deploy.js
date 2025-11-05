const { ethers } = require("hardhat");
const fs = require("fs");
const path = require("path");

// Production-safe deployment script for FlashArb contracts
// Implements all audit requirements: real logic only, comprehensive validation, production safety

async function main() {
    console.log("============================================================");
    console.log("FLASH ARBITRAGE CONTRACT DEPLOYMENT");
    console.log("============================================================");
    console.log("Network:", hre.network.name);
    console.log("Chain ID:", (await ethers.provider.getNetwork()).chainId);
    console.log("Deployer:", (await ethers.getSigners())[0].address);
    console.log("============================================================");

    // Get the deployer account
    const [deployer] = await ethers.getSigners();
    const deployerAddress = await deployer.getAddress();
    const balance = await ethers.provider.getBalance(deployerAddress);
    
    console.log("Deployer Address:", deployerAddress);
    console.log("Deployer Balance:", ethers.formatEther(balance), "ETH");
    
    // Validate minimum balance for deployment (skip for localhost/hardhat)
    const minBalance = ethers.parseEther("0.1"); // 0.1 ETH minimum
    if (balance < minBalance && hre.network.name !== "localhost" && hre.network.name !== "hardhat") {
        throw new Error(`Insufficient balance. Need at least ${ethers.formatEther(minBalance)} ETH`);
    }

    // Network-specific contract addresses
    const networkConfig = getNetworkConfig(hre.network.name);
    console.log("\nNetwork Configuration:");
    console.log("Aave Pool:", networkConfig.aavePool);
    console.log("Uniswap V3 Router:", networkConfig.uniswapV3Router);
    console.log("Sushiswap Router:", networkConfig.sushiswapRouter);

    // Validate all required addresses are set
    validateAddresses(networkConfig);

    // Deploy FlashArbUltimate contract
    console.log("\n============================================================");
    console.log("DEPLOYING FLASH ARBITRAGE CONTRACT");
    console.log("============================================================");

    const FlashArbUltimate = await ethers.getContractFactory("FlashArbUltimate");
    
    // Constructor parameters for FlashArbUltimate
    const minProfitWei = ethers.parseEther("0.001"); // 0.001 ETH minimum profit (testnet)
    const maxFailedAttempts = 5; // Max failed attempts before circuit breaker
    
    console.log("Constructor Parameters:");
    console.log("- Aave Pool:", networkConfig.aavePool);
    console.log("- Uniswap V3 Router:", networkConfig.uniswapV3Router);
    console.log("- Sushiswap Router:", networkConfig.sushiswapRouter);
    console.log("- Min Profit:", ethers.formatEther(minProfitWei), "ETH");
    console.log("- Max Failed Attempts:", maxFailedAttempts);

    // Estimate gas before deployment
    const deploymentData = FlashArbUltimate.interface.encodeDeploy([
        networkConfig.aavePool,
        networkConfig.uniswapV3Router,
        networkConfig.sushiswapRouter,
        minProfitWei,
        maxFailedAttempts
    ]);
    
    const gasEstimate = await ethers.provider.estimateGas({
        data: FlashArbUltimate.bytecode + deploymentData.slice(2)
    });
    
    const feeData = await ethers.provider.getFeeData();
    const gasPrice = feeData.gasPrice || feeData.maxFeePerGas || ethers.parseUnits("1", "gwei");
    
    console.log("Estimated Gas:", gasEstimate.toString());
    console.log("Estimated Cost:", ethers.formatEther(gasEstimate * gasPrice), "ETH");

    // Deploy the contract
    console.log("\nDeploying contract...");
    const flashArb = await FlashArbUltimate.deploy(
        networkConfig.aavePool,
        networkConfig.uniswapV3Router,
        networkConfig.sushiswapRouter,
        minProfitWei,
        maxFailedAttempts,
        {
            gasLimit: gasEstimate * 120n / 100n, // 20% buffer
        }
    );

    console.log("Transaction hash:", flashArb.deploymentTransaction().hash);
    console.log("Waiting for deployment confirmation...");
    
    await flashArb.waitForDeployment();
    const contractAddress = await flashArb.getAddress();
    
    console.log("✅ Contract deployed successfully!");
    console.log("Contract Address:", contractAddress);
    console.log("Gas Used:", flashArb.deploymentTransaction().gasLimit.toString());

    // Verify contract deployment
    console.log("\n============================================================");
    console.log("VERIFYING CONTRACT DEPLOYMENT");
    console.log("============================================================");

    // Check contract is properly deployed
    const code = await ethers.provider.getCode(contractAddress);
    if (code === "0x") {
        throw new Error("Contract deployment failed - no code at address");
    }
    console.log("✅ Contract code verified");

    // Verify contract state (FlashArbUltimate specific)
    const owner = await flashArb.owner();
    const minProfit = await flashArb.minProfitWei();
    const isPaused = await flashArb.paused();
    const isHealthy = await flashArb.isHealthy();

    console.log("Contract State:");
    console.log("- Owner:", owner);
    console.log("- Min Profit:", ethers.formatEther(minProfit), "ETH");
    console.log("- Paused:", isPaused);
    console.log("- Healthy:", isHealthy);

    if (owner !== deployerAddress) {
        throw new Error("Owner mismatch");
    }
    if (minProfit !== minProfitWei) {
        throw new Error("Min profit mismatch");
    }
    if (isPaused !== false) {
        throw new Error("Contract should not be paused on deployment");
    }
    if (isHealthy !== true) {
        throw new Error("Contract should be healthy on deployment");
    }

    console.log("✅ Contract state verified");

    // Save deployment information
    console.log("\n============================================================");
    console.log("SAVING DEPLOYMENT INFORMATION");
    console.log("============================================================");

    const deploymentInfo = {
        network: hre.network.name,
        chainId: (await ethers.provider.getNetwork()).chainId,
        contractAddress: contractAddress,
        deployer: deployerAddress,
        transactionHash: flashArb.deploymentTransaction().hash,
        gasUsed: flashArb.deploymentTransaction().gasLimit.toString(),
        blockNumber: await ethers.provider.getBlockNumber(),
        timestamp: new Date().toISOString(),
        constructorArgs: {
            aavePool: networkConfig.aavePool,
            uniswapV3Router: networkConfig.uniswapV3Router,
            sushiswapRouter: networkConfig.sushiswapRouter,
            permit2: networkConfig.permit2,
            minProfitWei: minProfitWei.toString()
        },
        verificationCommand: `npx hardhat verify --network ${hre.network.name} ${contractAddress} "${networkConfig.aavePool}" "${networkConfig.uniswapV3Router}" "${networkConfig.sushiswapRouter}" "${networkConfig.permit2}" "${minProfitWei.toString()}"`
    };

    // Create deployments directory if it doesn't exist
    const deploymentsDir = path.join(__dirname, "deployments");
    if (!fs.existsSync(deploymentsDir)) {
        fs.mkdirSync(deploymentsDir, { recursive: true });
    }

    // Save deployment info
    const deploymentFile = path.join(deploymentsDir, `${hre.network.name}.json`);
    fs.writeFileSync(deploymentFile, JSON.stringify(deploymentInfo, null, 2));
    console.log("✅ Deployment info saved to:", deploymentFile);

    // Update .env file with contract address
    updateEnvFile(contractAddress, hre.network.name);

    // Generate verification script
    const verifyScript = path.join(deploymentsDir, `verify-${hre.network.name}.sh`);
    fs.writeFileSync(verifyScript, `#!/bin/bash
# Contract verification script for ${hre.network.name}
# Generated on ${new Date().toISOString()}

echo "Verifying contract on ${hre.network.name}..."
${deploymentInfo.verificationCommand}

echo "Verification complete!"
`);
    fs.chmodSync(verifyScript, 0o755);
    console.log("✅ Verification script created:", verifyScript);

    // Final summary
    console.log("\n============================================================");
    console.log("DEPLOYMENT COMPLETED SUCCESSFULLY");
    console.log("============================================================");
    console.log("Contract Address:", contractAddress);
    console.log("Network:", hre.network.name);
    console.log("Block Explorer:", getBlockExplorerUrl(hre.network.name, contractAddress));
    console.log("Verification Command:", deploymentInfo.verificationCommand);
    console.log("============================================================");

    return {
        contractAddress,
        deploymentInfo
    };
}

function getNetworkConfig(networkName) {
    const configs = {
        mainnet: {
            aavePool: process.env.AAVE_POOL_ADDRESS || "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2",
            uniswapV3Router: process.env.UNISWAP_V3_ROUTER || "0xE592427A0AEce92De3Edee1F18E0157C05861564",
            sushiswapRouter: process.env.SUSHISWAP_ROUTER || "0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F",
            permit2: process.env.PERMIT2_ADDRESS || "0x000000000022D473030F116dDEE9F6B43aC78BA3",
        },
        sepolia: {
            aavePool: process.env.AAVE_POOL_ADDRESS || "0x6Ae43d3271ff6888e7Fc43Fd7321a503ff738951",
            uniswapV3Router: process.env.UNISWAP_V3_ROUTER || "0xE592427A0AEce92De3Edee1F18E0157C05861564",
            sushiswapRouter: process.env.SUSHISWAP_ROUTER || "0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506",
            permit2: process.env.PERMIT2_ADDRESS || "0x000000000022D473030F116dDEE9F6B43aC78BA3",
        },
        polygon: {
            aavePool: process.env.AAVE_POOL_ADDRESS || "0x794a61358D6845594F94dc1DB02A252b5b4814aD",
            uniswapV3Router: process.env.UNISWAP_V3_ROUTER || "0xE592427A0AEce92De3Edee1F18E0157C05861564",
            sushiswapRouter: process.env.SUSHISWAP_ROUTER || "0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506",
            permit2: process.env.PERMIT2_ADDRESS || "0x000000000022D473030F116dDEE9F6B43aC78BA3",
        },
        arbitrum: {
            aavePool: process.env.AAVE_POOL_ADDRESS || "0x794a61358D6845594F94dc1DB02A252b5b4814aD",
            uniswapV3Router: process.env.UNISWAP_V3_ROUTER || "0xE592427A0AEce92De3Edee1F18E0157C05861564",
            sushiswapRouter: process.env.SUSHISWAP_ROUTER || "0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506",
            permit2: process.env.PERMIT2_ADDRESS || "0x000000000022D473030F116dDEE9F6B43aC78BA3",
        },
        arbitrumSepolia: {
            // Official Aave V3 Arbitrum Sepolia addresses from https://github.com/bgd-labs/aave-address-book
            aavePool: process.env.AAVE_POOL_ADDRESS || "0xBfC91D59fdAA134A4ED45f7B584cAf96D7792Eff",
            // Uniswap V3 SwapRouter02 on Arbitrum Sepolia
            uniswapV3Router: process.env.UNISWAP_V3_ROUTER || "0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45",
            // Sushiswap V2 Router (fallback - may not exist on testnet)
            sushiswapRouter: process.env.SUSHISWAP_ROUTER || "0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506",
            // Permit2 (universal across chains)
            permit2: process.env.PERMIT2_ADDRESS || "0x000000000022D473030F116dDEE9F6B43aC78BA3",
        },
        optimism: {
            aavePool: process.env.AAVE_POOL_ADDRESS || "0x794a61358D6845594F94dc1DB02A252b5b4814aD",
            uniswapV3Router: process.env.UNISWAP_V3_ROUTER || "0xE592427A0AEce92De3Edee1F18E0157C05861564",
            sushiswapRouter: process.env.SUSHISWAP_ROUTER || "0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506",
            permit2: process.env.PERMIT2_ADDRESS || "0x000000000022D473030F116dDEE9F6B43aC78BA3",
        },
    };

    return configs[networkName] || configs.mainnet;
}

function validateAddresses(config) {
    const requiredAddresses = ['aavePool', 'uniswapV3Router', 'sushiswapRouter'];
    
    for (const address of requiredAddresses) {
        if (!config[address] || config[address] === "0x0000000000000000000000000000000000000000") {
            throw new Error(`Missing or invalid ${address} address: ${config[address]}`);
        }
        
        // Basic address validation
        if (!ethers.isAddress(config[address])) {
            throw new Error(`Invalid ${address} address format: ${config[address]}`);
        }
    }
}

function updateEnvFile(contractAddress, networkName) {
    const envFile = path.join(__dirname, ".env");
    let envContent = "";
    
    if (fs.existsSync(envFile)) {
        envContent = fs.readFileSync(envFile, "utf8");
    }
    
    // Update or add contract address
    const contractKey = `FLASH_ARB_ADDRESS_${networkName.toUpperCase()}`;
    const contractLine = `${contractKey}=${contractAddress}`;
    
    if (envContent.includes(contractKey)) {
        envContent = envContent.replace(new RegExp(`${contractKey}=.*`), contractLine);
    } else {
        envContent += `\n${contractLine}\n`;
    }
    
    // Also update the main contract address
    if (envContent.includes("FLASH_ARB_ADDRESS=")) {
        envContent = envContent.replace(/FLASH_ARB_ADDRESS=.*/, `FLASH_ARB_ADDRESS=${contractAddress}`);
    } else {
        envContent += `\nFLASH_ARB_ADDRESS=${contractAddress}\n`;
    }
    
    fs.writeFileSync(envFile, envContent);
    console.log("✅ .env file updated with contract address");
}

function getBlockExplorerUrl(networkName, address) {
    const explorers = {
        mainnet: `https://etherscan.io/address/${address}`,
        sepolia: `https://sepolia.etherscan.io/address/${address}`,
        goerli: `https://goerli.etherscan.io/address/${address}`,
        polygon: `https://polygonscan.com/address/${address}`,
        arbitrum: `https://arbiscan.io/address/${address}`,
        arbitrumSepolia: `https://sepolia.arbiscan.io/address/${address}`,
        optimism: `https://optimistic.etherscan.io/address/${address}`,
    };
    
    return explorers[networkName] || explorers.mainnet;
}

// Handle errors gracefully
main()
    .then(() => process.exit(0))
    .catch((error) => {
        console.error("❌ Deployment failed:", error);
        process.exit(1);
    });
