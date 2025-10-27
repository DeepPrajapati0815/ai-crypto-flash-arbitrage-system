const { ethers } = require("hardhat");
const fs = require("fs");
const path = require("path");

// Contract verification script for FlashArb contracts
// Implements comprehensive verification with retry logic and validation

async function main() {
    console.log("============================================================");
    console.log("CONTRACT VERIFICATION");
    console.log("============================================================");
    console.log("Network:", hre.network.name);
    console.log("Chain ID:", (await ethers.provider.getNetwork()).chainId);
    console.log("============================================================");

    // Load deployment information
    const deploymentFile = path.join(__dirname, "..", "deployments", `${hre.network.name}.json`);
    
    if (!fs.existsSync(deploymentFile)) {
        throw new Error(`Deployment file not found: ${deploymentFile}`);
    }

    const deploymentInfo = JSON.parse(fs.readFileSync(deploymentFile, "utf8"));
    console.log("Contract Address:", deploymentInfo.contractAddress);
    console.log("Deployment Block:", deploymentInfo.blockNumber);

    // Verify contract is deployed
    const code = await ethers.provider.getCode(deploymentInfo.contractAddress);
    if (code === "0x") {
        throw new Error("No contract code found at address");
    }
    console.log("✅ Contract code verified");

    // Wait for contract to be indexed (if recently deployed)
    const currentBlock = await ethers.provider.getBlockNumber();
    const blocksSinceDeployment = currentBlock - deploymentInfo.blockNumber;
    
    if (blocksSinceDeployment < 5) {
        console.log(`⏳ Waiting for contract to be indexed (${blocksSinceDeployment} blocks since deployment)...`);
        await new Promise(resolve => setTimeout(resolve, 30000)); // Wait 30 seconds
    }

    // Attempt verification with retry logic
    console.log("\n============================================================");
    console.log("VERIFYING CONTRACT ON BLOCK EXPLORER");
    console.log("============================================================");

    const maxRetries = 3;
    let verificationSuccess = false;
    let lastError = null;

    for (let attempt = 1; attempt <= maxRetries; attempt++) {
        console.log(`\nVerification attempt ${attempt}/${maxRetries}...`);
        
        try {
            await hre.run("verify:verify", {
                address: deploymentInfo.contractAddress,
                constructorArguments: [
                    deploymentInfo.constructorArgs.aavePool,
                    deploymentInfo.constructorArgs.uniswapV3Router,
                    deploymentInfo.constructorArgs.sushiswapRouter,
                    deploymentInfo.constructorArgs.permit2,
                    deploymentInfo.constructorArgs.minProfitWei
                ],
                contract: "contracts/FlashArbProductionSafe.sol:FlashArbProductionSafe"
            });
            
            console.log("✅ Contract verification successful!");
            verificationSuccess = true;
            break;
            
        } catch (error) {
            lastError = error;
            console.log(`❌ Verification attempt ${attempt} failed:`, error.message);
            
            if (attempt < maxRetries) {
                console.log("⏳ Waiting 30 seconds before retry...");
                await new Promise(resolve => setTimeout(resolve, 30000));
            }
        }
    }

    if (!verificationSuccess) {
        console.log("\n❌ Contract verification failed after all attempts");
        console.log("Last error:", lastError.message);
        
        // Generate manual verification command
        console.log("\n============================================================");
        console.log("MANUAL VERIFICATION COMMAND");
        console.log("============================================================");
        console.log("You can try manual verification using:");
        console.log(deploymentInfo.verificationCommand);
        
        // Save verification failure info
        const failureInfo = {
            network: hre.network.name,
            contractAddress: deploymentInfo.contractAddress,
            failureTime: new Date().toISOString(),
            lastError: lastError.message,
            manualCommand: deploymentInfo.verificationCommand
        };
        
        const failureFile = path.join(__dirname, "..", "deployments", `verification-failure-${hre.network.name}.json`);
        fs.writeFileSync(failureFile, JSON.stringify(failureInfo, null, 2));
        console.log("Failure info saved to:", failureFile);
        
        process.exit(1);
    }

    // Verify contract functions are accessible
    console.log("\n============================================================");
    console.log("VERIFYING CONTRACT FUNCTIONS");
    console.log("============================================================");

    try {
        const contract = await ethers.getContractAt("FlashArbProductionSafe", deploymentInfo.contractAddress);
        
        // Test basic contract functions
        const owner = await contract.owner();
        const minProfit = await contract.getMinProfitWei();
        const isPaused = await contract.isEmergencyPaused();
        const bundleOnlyMode = await contract.bundleOnlyMode();
        const failedAttempts = await contract.getFailedAttempts();

        console.log("Contract Function Tests:");
        console.log("✅ Owner:", owner);
        console.log("✅ Min Profit:", ethers.formatEther(minProfit), "ETH");
        console.log("✅ Emergency Paused:", isPaused);
        console.log("✅ Bundle Only Mode:", bundleOnlyMode);
        console.log("✅ Failed Attempts:", failedAttempts.toString());

        // Verify constructor parameters
        const expectedMinProfit = ethers.parseEther("0.01");
        if (minProfit !== expectedMinProfit) {
            throw new Error(`Min profit mismatch: expected ${ethers.formatEther(expectedMinProfit)}, got ${ethers.formatEther(minProfit)}`);
        }

        console.log("✅ Constructor parameters verified");

    } catch (error) {
        console.log("❌ Contract function verification failed:", error.message);
        throw error;
    }

    // Update deployment info with verification status
    deploymentInfo.verified = true;
    deploymentInfo.verificationTime = new Date().toISOString();
    deploymentInfo.blockExplorerUrl = getBlockExplorerUrl(hre.network.name, deploymentInfo.contractAddress);
    
    fs.writeFileSync(deploymentFile, JSON.stringify(deploymentInfo, null, 2));
    console.log("✅ Deployment info updated with verification status");

    // Final summary
    console.log("\n============================================================");
    console.log("VERIFICATION COMPLETED SUCCESSFULLY");
    console.log("============================================================");
    console.log("Contract Address:", deploymentInfo.contractAddress);
    console.log("Network:", hre.network.name);
    console.log("Block Explorer:", deploymentInfo.blockExplorerUrl);
    console.log("Verification Status: ✅ VERIFIED");
    console.log("============================================================");

    return {
        contractAddress: deploymentInfo.contractAddress,
        verified: true,
        blockExplorerUrl: deploymentInfo.blockExplorerUrl
    };
}

function getBlockExplorerUrl(networkName, address) {
    const explorers = {
        mainnet: `https://etherscan.io/address/${address}`,
        sepolia: `https://sepolia.etherscan.io/address/${address}`,
        goerli: `https://goerli.etherscan.io/address/${address}`,
        polygon: `https://polygonscan.com/address/${address}`,
        arbitrum: `https://arbiscan.io/address/${address}`,
        optimism: `https://optimistic.etherscan.io/address/${address}`,
    };
    
    return explorers[networkName] || explorers.mainnet;
}

// Handle errors gracefully
main()
    .then(() => process.exit(0))
    .catch((error) => {
        console.error("❌ Verification failed:", error);
        process.exit(1);
    });
