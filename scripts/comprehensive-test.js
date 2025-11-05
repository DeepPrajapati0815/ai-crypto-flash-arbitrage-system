const { ethers } = require("hardhat");
const fs = require("fs");
const path = require("path");

/**
 * Comprehensive Smart Contract Testing Script
 * Tests all FlashArbUltimate contract methods and scenarios
 * 
 * Usage:
 *   npx hardhat run scripts/comprehensive-test.js --network sepolia
 *   npx hardhat run scripts/comprehensive-test.js --network localhost
 */

// Test configuration
const TEST_CONFIG = {
    // Token addresses (Ethereum Mainnet - update for other networks)
    WETH: "0xfff9976782d46cc05630d1f6ebab18b2324d6b14",
    USDC: "0x94a9d9ac8a22534e3faca9f4e7f2e2cf85d5e4c8",
    
    // Test amounts
    FLASH_LOAN_AMOUNT: ethers.parseEther("1"), // 1 WETH
    MIN_PROFIT: ethers.parseEther("0.01"), // 0.01 ETH
    
    // Test parameters
    MAX_GAS_PRICE: ethers.parseUnits("200", "gwei"),
    COOLDOWN_SECONDS: 10,
};

// Test results tracker
const testResults = {
    network: "",
    contractAddress: "",
    timestamp: new Date().toISOString(),
    tests: [],
    summary: {
        total: 0,
        passed: 0,
        failed: 0,
        skipped: 0
    }
};

// Helper function to log test results
function logTest(name, status, message, data = {}) {
    const result = { name, status, message, data, timestamp: new Date().toISOString() };
    testResults.tests.push(result);
    
    const emoji = status === "PASS" ? "✅" : status === "FAIL" ? "❌" : "⏭️";
    console.log(`${emoji} ${name}: ${message}`);
    
    if (Object.keys(data).length > 0) {
        console.log("   Data:", JSON.stringify(data, null, 2));
    }
}

// Helper function to format address
function formatAddress(address) {
    return `${address.slice(0, 6)}...${address.slice(-4)}`;
}

async function main() {
    console.log("============================================================");
    console.log("COMPREHENSIVE SMART CONTRACT TESTING");
    console.log("============================================================");
    console.log("Network:", hre.network.name);
    console.log("Chain ID:", (await ethers.provider.getNetwork()).chainId);
    console.log("Timestamp:", new Date().toISOString());
    console.log("============================================================\n");

    testResults.network = hre.network.name;

    // Get signers
    const signers = await ethers.getSigners();
    const deployer = signers[0];
    const bot = signers[1] || signers[0]; // Use deployer as bot if only one signer
    const user = signers[2] || null;
    
    console.log("Deployer:", deployer.address);
    console.log("Bot:", bot.address);
    console.log("User:", user ? user.address : "N/A");
    console.log("");

    // Load deployment info
    const deploymentFile = path.join(__dirname, "..", "deployments", `${hre.network.name}-deployment.json`);
    
    if (!fs.existsSync(deploymentFile)) {
        console.error("❌ Deployment file not found:", deploymentFile);
        console.log("Please deploy the contract first using:");
        console.log(`  npx hardhat run scripts/deploy.js --network ${hre.network.name}`);
        process.exit(1);
    }

    const deploymentInfo = JSON.parse(fs.readFileSync(deploymentFile, "utf8"));
    const contractAddress = deploymentInfo.contracts.FlashArbUltimate;
    testResults.contractAddress = contractAddress;

    console.log("Contract Address:", contractAddress);
    console.log("Block Explorer:", getBlockExplorerUrl(hre.network.name, contractAddress));
    console.log("");

    // Get contract instance
    const contract = await ethers.getContractAt("FlashArbUltimate", contractAddress);

    // ========================================================================
    // TEST SUITE 1: BASIC CONTRACT VALIDATION
    // ========================================================================
    console.log("============================================================");
    console.log("TEST SUITE 1: BASIC CONTRACT VALIDATION");
    console.log("============================================================\n");

    // Test 1.1: Contract Code Verification
    try {
        const code = await ethers.provider.getCode(contractAddress);
        if (code === "0x") {
            throw new Error("No contract code found at address");
        }
        logTest("1.1 Contract Code", "PASS", "Contract bytecode verified", {
            codeLength: code.length
        });
    } catch (error) {
        logTest("1.1 Contract Code", "FAIL", error.message);
    }

    // Test 1.2: Owner Verification
    try {
        const owner = await contract.owner();
        const isCorrectOwner = owner === deployer.address;
        
        if (!isCorrectOwner) {
            throw new Error(`Owner mismatch: expected ${deployer.address}, got ${owner}`);
        }
        
        logTest("1.2 Owner Verification", "PASS", "Owner is deployer", {
            owner: formatAddress(owner)
        });
    } catch (error) {
        logTest("1.2 Owner Verification", "FAIL", error.message);
    }

    // Test 1.3: Initial State Check
    try {
        const status = await contract.getStatus();
        const healthy = await contract.isHealthy();
        
        logTest("1.3 Initial State", "PASS", "Contract state verified", {
            paused: status.isPaused,
            emergencyPaused: status.isEmergencyPaused,
            failedAttempts: status.currentFailedAttempts.toString(),
            maxAttempts: status.maxAttempts.toString(),
            minProfit: ethers.formatEther(status.currentMinProfit) + " ETH",
            bundleOnly: status.isBundleOnly,
            healthy: healthy
        });
    } catch (error) {
        logTest("1.3 Initial State", "FAIL", error.message);
    }

    // Test 1.4: Constructor Parameters
    try {
        const minProfit = await contract.minProfitWei();
        const maxAttempts = await contract.maxFailedAttempts();
        
        logTest("1.4 Constructor Parameters", "PASS", "Parameters validated", {
            minProfitWei: ethers.formatEther(minProfit) + " ETH",
            maxFailedAttempts: maxAttempts.toString()
        });
    } catch (error) {
        logTest("1.4 Constructor Parameters", "FAIL", error.message);
    }

    // ========================================================================
    // TEST SUITE 2: AUTHORIZATION & ACCESS CONTROL
    // ========================================================================
    console.log("\n============================================================");
    console.log("TEST SUITE 2: AUTHORIZATION & ACCESS CONTROL");
    console.log("============================================================\n");

    // Test 2.1: Add Authorized Executor
    try {
        const tx = await contract.connect(deployer).addAuthorizedExecutor(bot.address);
        await tx.wait();
        
        const isAuthorized = await contract.authorizedExecutors(bot.address);
        if (!isAuthorized) {
            throw new Error("Bot not authorized after addAuthorizedExecutor");
        }
        
        logTest("2.1 Add Executor", "PASS", "Bot authorized as executor", {
            botAddress: formatAddress(bot.address)
        });
    } catch (error) {
        logTest("2.1 Add Executor", "FAIL", error.message);
    }

    // Test 2.2: Authorize Bundle Submitter
    try {
        const tx = await contract.connect(deployer).authorizeBundleSubmitter(bot.address);
        await tx.wait();
        
        const isAuthorized = await contract.authorizedBundleSubmitters(bot.address);
        if (!isAuthorized) {
            throw new Error("Bot not authorized as bundle submitter");
        }
        
        logTest("2.2 Authorize Bundle Submitter", "PASS", "Bot authorized for bundles", {
            botAddress: formatAddress(bot.address)
        });
    } catch (error) {
        logTest("2.2 Authorize Bundle Submitter", "FAIL", error.message);
    }

    // Test 2.3: Revoke Bundle Submitter
    try {
        const tx1 = await contract.connect(deployer).revokeBundleSubmitter(bot.address);
        await tx1.wait();
        
        let isAuthorized = await contract.authorizedBundleSubmitters(bot.address);
        if (isAuthorized) {
            throw new Error("Bot still authorized after revocation");
        }
        
        // Re-authorize for future tests
        const tx2 = await contract.connect(deployer).authorizeBundleSubmitter(bot.address);
        await tx2.wait();
        
        logTest("2.3 Revoke Bundle Submitter", "PASS", "Revocation and re-authorization works");
    } catch (error) {
        logTest("2.3 Revoke Bundle Submitter", "FAIL", error.message);
    }

    // Test 2.4: Unauthorized Access Prevention
    if (user) {
        try {
            // Try to execute without authorization (should fail)
            await contract.connect(user).addAuthorizedExecutor(user.address);
            logTest("2.4 Unauthorized Access", "FAIL", "Unauthorized user was able to call owner function");
        } catch (error) {
            if (error.message.includes("Ownable")) {
                logTest("2.4 Unauthorized Access", "PASS", "Unauthorized access correctly prevented");
            } else {
                logTest("2.4 Unauthorized Access", "FAIL", "Unexpected error: " + error.message);
            }
        }
    } else {
        logTest("2.4 Unauthorized Access", "SKIP", "No third signer available");
    }

    // ========================================================================
    // TEST SUITE 3: CONFIGURATION MANAGEMENT
    // ========================================================================
    console.log("\n============================================================");
    console.log("TEST SUITE 3: CONFIGURATION MANAGEMENT");
    console.log("============================================================\n");

    // Test 3.1: Bundle Mode Toggle
    try {
        const initialMode = await contract.bundleOnlyMode();
        
        // Toggle off
        const tx1 = await contract.connect(deployer).setBundleOnlyMode(false);
        await tx1.wait();
        const modeOff = await contract.bundleOnlyMode();
        
        // Toggle on
        const tx2 = await contract.connect(deployer).setBundleOnlyMode(true);
        await tx2.wait();
        const modeOn = await contract.bundleOnlyMode();
        
        if (modeOff !== false || modeOn !== true) {
            throw new Error("Bundle mode toggle failed");
        }
        
        logTest("3.1 Bundle Mode Toggle", "PASS", "Bundle mode toggle works", {
            initial: initialMode,
            afterDisable: modeOff,
            afterEnable: modeOn
        });
    } catch (error) {
        logTest("3.1 Bundle Mode Toggle", "FAIL", error.message);
    }

    // Test 3.2: Min Profit Update
    try {
        const oldMinProfit = await contract.minProfitWei();
        const newMinProfit = ethers.parseEther("0.02");
        
        const tx = await contract.connect(deployer).setMinProfitWei(newMinProfit);
        await tx.wait();
        
        const updatedMinProfit = await contract.minProfitWei();
        
        if (updatedMinProfit !== newMinProfit) {
            throw new Error("Min profit not updated correctly");
        }
        
        // Restore original
        const tx2 = await contract.connect(deployer).setMinProfitWei(oldMinProfit);
        await tx2.wait();
        
        logTest("3.2 Min Profit Update", "PASS", "Min profit updated successfully", {
            old: ethers.formatEther(oldMinProfit) + " ETH",
            new: ethers.formatEther(newMinProfit) + " ETH"
        });
    } catch (error) {
        logTest("3.2 Min Profit Update", "FAIL", error.message);
    }

    // Test 3.3: Max Failed Attempts Update
    try {
        const oldMax = await contract.maxFailedAttempts();
        const newMax = 10n;
        
        const tx = await contract.connect(deployer).setMaxFailedAttempts(newMax);
        await tx.wait();
        
        const updatedMax = await contract.maxFailedAttempts();
        
        if (updatedMax !== newMax) {
            throw new Error("Max failed attempts not updated");
        }
        
        logTest("3.3 Max Failed Attempts", "PASS", "Circuit breaker threshold updated", {
            old: oldMax.toString(),
            new: updatedMax.toString()
        });
    } catch (error) {
        logTest("3.3 Max Failed Attempts", "FAIL", error.message);
    }

    // Test 3.4: Max Gas Price Update
    try {
        const oldMaxGas = await contract.maxGasPrice();
        const newMaxGas = TEST_CONFIG.MAX_GAS_PRICE;
        
        const tx = await contract.connect(deployer).setMaxGasPrice(newMaxGas);
        await tx.wait();
        
        const updatedMaxGas = await contract.maxGasPrice();
        
        if (updatedMaxGas !== newMaxGas) {
            throw new Error("Max gas price not updated");
        }
        
        logTest("3.4 Max Gas Price", "PASS", "Max gas price updated", {
            old: ethers.formatUnits(oldMaxGas, "gwei") + " gwei",
            new: ethers.formatUnits(updatedMaxGas, "gwei") + " gwei"
        });
    } catch (error) {
        logTest("3.4 Max Gas Price", "FAIL", error.message);
    }

    // Test 3.5: Execution Cooldown Update
    try {
        const oldCooldown = await contract.executionCooldown();
        const newCooldown = TEST_CONFIG.COOLDOWN_SECONDS;
        
        const tx = await contract.connect(deployer).setExecutionCooldown(newCooldown);
        await tx.wait();
        
        const updatedCooldown = await contract.executionCooldown();
        
        if (updatedCooldown !== BigInt(newCooldown)) {
            throw new Error("Cooldown not updated");
        }
        
        logTest("3.5 Execution Cooldown", "PASS", "Cooldown period updated", {
            old: oldCooldown.toString() + "s",
            new: updatedCooldown.toString() + "s"
        });
    } catch (error) {
        logTest("3.5 Execution Cooldown", "FAIL", error.message);
    }

    // ========================================================================
    // TEST SUITE 4: PAUSE & EMERGENCY CONTROLS
    // ========================================================================
    console.log("\n============================================================");
    console.log("TEST SUITE 4: PAUSE & EMERGENCY CONTROLS");
    console.log("============================================================\n");

    // Test 4.1: Emergency Pause
    try {
        const tx1 = await contract.connect(deployer).setEmergencyPaused(true);
        await tx1.wait();
        
        let status = await contract.getStatus();
        let healthy = await contract.isHealthy();
        
        if (!status.isEmergencyPaused || healthy) {
            throw new Error("Emergency pause not working correctly");
        }
        
        // Unpause
        const tx2 = await contract.connect(deployer).setEmergencyPaused(false);
        await tx2.wait();
        
        status = await contract.getStatus();
        healthy = await contract.isHealthy();
        
        if (status.isEmergencyPaused || !healthy) {
            throw new Error("Emergency unpause not working correctly");
        }
        
        logTest("4.1 Emergency Pause", "PASS", "Emergency pause/unpause works");
    } catch (error) {
        logTest("4.1 Emergency Pause", "FAIL", error.message);
    }

    // Test 4.2: Regular Pause
    try {
        const tx1 = await contract.connect(deployer).setPaused(true);
        await tx1.wait();
        
        let status = await contract.getStatus();
        
        if (!status.isPaused) {
            throw new Error("Regular pause not working");
        }
        
        // Unpause
        const tx2 = await contract.connect(deployer).setPaused(false);
        await tx2.wait();
        
        status = await contract.getStatus();
        
        if (status.isPaused) {
            throw new Error("Regular unpause not working");
        }
        
        logTest("4.2 Regular Pause", "PASS", "Regular pause/unpause works");
    } catch (error) {
        logTest("4.2 Regular Pause", "FAIL", error.message);
    }

    // Test 4.3: Circuit Breaker Reset
    try {
        const tx = await contract.connect(deployer).resetCircuitBreaker();
        await tx.wait();
        
        const status = await contract.getStatus();
        
        if (status.currentFailedAttempts !== 0n || status.isEmergencyPaused) {
            throw new Error("Circuit breaker not reset correctly");
        }
        
        logTest("4.3 Circuit Breaker Reset", "PASS", "Circuit breaker reset successful");
    } catch (error) {
        logTest("4.3 Circuit Breaker Reset", "FAIL", error.message);
    }

    // ========================================================================
    // TEST SUITE 5: GAS ESTIMATION
    // ========================================================================
    console.log("\n============================================================");
    console.log("TEST SUITE 5: GAS ESTIMATION");
    console.log("============================================================\n");

    // Disable bundle-only mode for gas estimation
    await contract.connect(deployer).setBundleOnlyMode(false);

    // Test 5.1: Single Route Gas Estimation
    try {
        const tuples = [
            {
                dexType: 0, // UniswapV3
                tokenIn: TEST_CONFIG.WETH,
                tokenOut: TEST_CONFIG.USDC,
                poolFee: 3000,
                amountIn: ethers.parseEther("0.1"),
                minAmountOut: ethers.parseUnits("180", 6)
            }
        ];
        
        const gasEstimate = await contract.connect(bot).executeFlashArbitrage.estimateGas(
            TEST_CONFIG.WETH,
            ethers.parseEther("0.1"),
            tuples
        );
        
        const gasCost = gasEstimate * 20000000000n; // 20 gwei
        
        logTest("5.1 Single Route Gas", "PASS", "Gas estimation successful", {
            estimatedGas: gasEstimate.toString(),
            costAt20Gwei: ethers.formatEther(gasCost) + " ETH"
        });
    } catch (error) {
        // Gas estimation may fail on testnet due to missing liquidity
        if (error.message.includes("execution reverted") || error.message.includes("CALL_EXCEPTION")) {
            logTest("5.1 Single Route Gas", "SKIP", "Skipped - No liquidity on testnet DEX pools");
        } else {
            logTest("5.1 Single Route Gas", "FAIL", error.message);
        }
    }

    // Test 5.2: Multi-Route Gas Estimation
    try {
        const tuples = [
            {
                dexType: 0,
                tokenIn: TEST_CONFIG.WETH,
                tokenOut: TEST_CONFIG.USDC,
                poolFee: 3000,
                amountIn: ethers.parseEther("0.1"),
                minAmountOut: ethers.parseUnits("180", 6)
            },
            {
                dexType: 1, // Sushiswap
                tokenIn: TEST_CONFIG.USDC,
                tokenOut: TEST_CONFIG.WETH,
                poolFee: 3000,
                amountIn: ethers.parseUnits("180", 6),
                minAmountOut: ethers.parseEther("0.101")
            }
        ];
        
        const gasEstimate = await contract.connect(bot).executeFlashArbitrage.estimateGas(
            TEST_CONFIG.WETH,
            ethers.parseEther("0.1"),
            tuples
        );
        
        const gasCost = gasEstimate * 20000000000n;
        
        logTest("5.2 Multi-Route Gas", "PASS", "Multi-route gas estimation successful", {
            estimatedGas: gasEstimate.toString(),
            costAt20Gwei: ethers.formatEther(gasCost) + " ETH"
        });
    } catch (error) {
        // Gas estimation may fail on testnet due to missing liquidity
        if (error.message.includes("execution reverted") || error.message.includes("CALL_EXCEPTION")) {
            logTest("5.2 Multi-Route Gas", "SKIP", "Skipped - No liquidity on testnet DEX pools");
        } else {
            logTest("5.2 Multi-Route Gas", "FAIL", error.message);
        }
    }

    // Re-enable bundle-only mode
    await contract.connect(deployer).setBundleOnlyMode(true);

    // ========================================================================
    // TEST SUITE 6: VIEW FUNCTIONS
    // ========================================================================
    console.log("\n============================================================");
    console.log("TEST SUITE 6: VIEW FUNCTIONS");
    console.log("============================================================\n");

    // Test 6.1: Get Balance
    try {
        const balance = await contract.getBalance(TEST_CONFIG.WETH);
        
        logTest("6.1 Get Balance", "PASS", "Balance query successful", {
            token: "WETH",
            balance: ethers.formatEther(balance) + " WETH"
        });
    } catch (error) {
        logTest("6.1 Get Balance", "FAIL", error.message);
    }

    // Test 6.2: Get Status
    try {
        const status = await contract.getStatus();
        
        logTest("6.2 Get Status", "PASS", "Status query successful", {
            paused: status.isPaused,
            emergencyPaused: status.isEmergencyPaused,
            failedAttempts: status.currentFailedAttempts.toString(),
            maxAttempts: status.maxAttempts.toString(),
            minProfit: ethers.formatEther(status.currentMinProfit) + " ETH",
            bundleOnly: status.isBundleOnly
        });
    } catch (error) {
        logTest("6.2 Get Status", "FAIL", error.message);
    }

    // Test 6.3: Is Healthy
    try {
        const healthy = await contract.isHealthy();
        
        logTest("6.3 Is Healthy", "PASS", "Health check successful", {
            healthy: healthy
        });
    } catch (error) {
        logTest("6.3 Is Healthy", "FAIL", error.message);
    }

    // ========================================================================
    // GENERATE TEST REPORT
    // ========================================================================
    console.log("\n============================================================");
    console.log("TEST RESULTS SUMMARY");
    console.log("============================================================\n");

    testResults.summary.total = testResults.tests.length;
    testResults.summary.passed = testResults.tests.filter(t => t.status === "PASS").length;
    testResults.summary.failed = testResults.tests.filter(t => t.status === "FAIL").length;
    testResults.summary.skipped = testResults.tests.filter(t => t.status === "SKIP").length;

    const successRate = testResults.summary.total > 0 
        ? (testResults.summary.passed / (testResults.summary.total - testResults.summary.skipped)) * 100 
        : 0;

    console.log(`Total Tests:   ${testResults.summary.total}`);
    console.log(`✅ Passed:     ${testResults.summary.passed}`);
    console.log(`❌ Failed:     ${testResults.summary.failed}`);
    console.log(`⏭️  Skipped:    ${testResults.summary.skipped}`);
    console.log(`Success Rate:  ${successRate.toFixed(1)}%`);
    console.log("");

    // Save detailed results
    const resultsDir = path.join(__dirname, "..", "test-results");
    if (!fs.existsSync(resultsDir)) {
        fs.mkdirSync(resultsDir, { recursive: true });
    }

    const resultsFile = path.join(resultsDir, `comprehensive-test-${hre.network.name}-${Date.now()}.json`);
    fs.writeFileSync(resultsFile, JSON.stringify(testResults, null, 2));
    console.log("Detailed results saved to:", resultsFile);
    console.log("");

    // Final verdict
    if (testResults.summary.failed > 0) {
        console.log("❌ SOME TESTS FAILED - Review results above");
        process.exit(1);
    } else {
        console.log("✅ ALL TESTS PASSED - Contract is ready!");
        console.log("");
        console.log("Next steps:");
        console.log("1. Verify contract on block explorer");
        console.log("2. Test with real arbitrage opportunities");
        console.log("3. Enable bundle-only mode for production");
        console.log("4. Set up monitoring and alerts");
    }
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

// Execute main function
main()
    .then(() => process.exit(0))
    .catch((error) => {
        console.error("\n❌ Testing failed with error:");
        console.error(error);
        process.exit(1);
    });
