const { ethers } = require("hardhat");
const fs = require("fs");
const path = require("path");

// Comprehensive contract testing and validation script
// Implements production-ready testing with real contract interactions

async function main() {
    console.log("============================================================");
    console.log("CONTRACT VALIDATION AND TESTING");
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

    // Get contract instance
    const FlashArbProductionSafe = await ethers.getContractFactory("FlashArbProductionSafe");
    const contract = FlashArbProductionSafe.attach(deploymentInfo.contractAddress);

    // Test results tracking
    const testResults = {
        network: hre.network.name,
        contractAddress: deploymentInfo.contractAddress,
        testTime: new Date().toISOString(),
        tests: {}
    };

    console.log("\n============================================================");
    console.log("BASIC CONTRACT VALIDATION");
    console.log("============================================================");

    // Test 1: Contract Deployment Validation
    try {
        const code = await ethers.provider.getCode(deploymentInfo.contractAddress);
        if (code === "0x") {
            throw new Error("No contract code found");
        }
        console.log("✅ Contract code exists");
        testResults.tests.contractCode = { status: "PASS", message: "Contract code verified" };
    } catch (error) {
        console.log("❌ Contract code validation failed:", error.message);
        testResults.tests.contractCode = { status: "FAIL", message: error.message };
    }

    // Test 2: Owner Validation
    try {
        const owner = await contract.owner();
        const [deployer] = await ethers.getSigners();
        const deployerAddress = await deployer.getAddress();
        
        if (owner !== deployerAddress) {
            throw new Error(`Owner mismatch: expected ${deployerAddress}, got ${owner}`);
        }
        console.log("✅ Owner validation passed");
        testResults.tests.owner = { status: "PASS", message: `Owner: ${owner}` };
    } catch (error) {
        console.log("❌ Owner validation failed:", error.message);
        testResults.tests.owner = { status: "FAIL", message: error.message };
    }

    // Test 3: Constructor Parameters Validation
    try {
        const minProfit = await contract.getMinProfitWei();
        const expectedMinProfit = ethers.parseEther("0.01");
        
        if (minProfit !== expectedMinProfit) {
            throw new Error(`Min profit mismatch: expected ${ethers.formatEther(expectedMinProfit)}, got ${ethers.formatEther(minProfit)}`);
        }
        console.log("✅ Constructor parameters validated");
        testResults.tests.constructorParams = { status: "PASS", message: `Min profit: ${ethers.formatEther(minProfit)} ETH` };
    } catch (error) {
        console.log("❌ Constructor parameters validation failed:", error.message);
        testResults.tests.constructorParams = { status: "FAIL", message: error.message };
    }

    // Test 4: Initial State Validation
    try {
        const isPaused = await contract.isEmergencyPaused();
        const bundleOnlyMode = await contract.bundleOnlyMode();
        const failedAttempts = await contract.getFailedAttempts();

        if (isPaused !== false) {
            throw new Error("Contract should not be paused on deployment");
        }
        if (bundleOnlyMode !== true) {
            throw new Error("Bundle only mode should be enabled on deployment");
        }
        if (failedAttempts !== 0n) {
            throw new Error("Failed attempts should be 0 on deployment");
        }

        console.log("✅ Initial state validated");
        testResults.tests.initialState = { 
            status: "PASS", 
            message: `Paused: ${isPaused}, Bundle Only: ${bundleOnlyMode}, Failed Attempts: ${failedAttempts}` 
        };
    } catch (error) {
        console.log("❌ Initial state validation failed:", error.message);
        testResults.tests.initialState = { status: "FAIL", message: error.message };
    }

    console.log("\n============================================================");
    console.log("FUNCTIONALITY TESTING");
    console.log("============================================================");

    // Test 5: Emergency Pause Functionality
    try {
        const [owner] = await ethers.getSigners();
        
        // Test emergency pause
        const pauseTx = await contract.connect(owner).emergencyPause("Testing emergency pause");
        await pauseTx.wait();
        
        const isPausedAfter = await contract.isEmergencyPaused();
        if (isPausedAfter !== true) {
            throw new Error("Emergency pause did not work");
        }
        
        // Test emergency unpause
        const unpauseTx = await contract.connect(owner).emergencyUnpause();
        await unpauseTx.wait();
        
        const isPausedAfterUnpause = await contract.isEmergencyPaused();
        if (isPausedAfterUnpause !== false) {
            throw new Error("Emergency unpause did not work");
        }

        console.log("✅ Emergency pause/unpause functionality works");
        testResults.tests.emergencyPause = { status: "PASS", message: "Emergency pause/unpause tested successfully" };
    } catch (error) {
        console.log("❌ Emergency pause functionality test failed:", error.message);
        testResults.tests.emergencyPause = { status: "FAIL", message: error.message };
    }

    // Test 6: Bundle Mode Configuration
    try {
        const [owner] = await ethers.getSigners();
        
        // Test disabling bundle only mode
        const disableTx = await contract.connect(owner).setBundleOnlyMode(false);
        await disableTx.wait();
        
        const bundleModeDisabled = await contract.bundleOnlyMode();
        if (bundleModeDisabled !== false) {
            throw new Error("Bundle mode disable did not work");
        }
        
        // Test re-enabling bundle only mode
        const enableTx = await contract.connect(owner).setBundleOnlyMode(true);
        await enableTx.wait();
        
        const bundleModeEnabled = await contract.bundleOnlyMode();
        if (bundleModeEnabled !== true) {
            throw new Error("Bundle mode enable did not work");
        }

        console.log("✅ Bundle mode configuration works");
        testResults.tests.bundleMode = { status: "PASS", message: "Bundle mode configuration tested successfully" };
    } catch (error) {
        console.log("❌ Bundle mode configuration test failed:", error.message);
        testResults.tests.bundleMode = { status: "FAIL", message: error.message };
    }

    // Test 7: Authorization Functions (if not on mainnet)
    if (hre.network.name !== "mainnet") {
        try {
            const [owner, testAccount] = await ethers.getSigners();
            
            // Test authorizing bundle submitter
            const authorizeTx = await contract.connect(owner).authorizeBundleSubmitter(testAccount.address);
            await authorizeTx.wait();
            
            const isAuthorized = await contract.authorizedBundleSubmitters(testAccount.address);
            if (isAuthorized !== true) {
                throw new Error("Bundle submitter authorization did not work");
            }
            
            // Test revoking authorization
            const revokeTx = await contract.connect(owner).revokeBundleSubmitter(testAccount.address);
            await revokeTx.wait();
            
            const isRevoked = await contract.authorizedBundleSubmitters(testAccount.address);
            if (isRevoked !== false) {
                throw new Error("Bundle submitter revocation did not work");
            }

            console.log("✅ Authorization functions work");
            testResults.tests.authorization = { status: "PASS", message: "Authorization functions tested successfully" };
        } catch (error) {
            console.log("❌ Authorization functions test failed:", error.message);
            testResults.tests.authorization = { status: "FAIL", message: error.message };
        }
    } else {
        console.log("⏭️  Skipping authorization test on mainnet");
        testResults.tests.authorization = { status: "SKIP", message: "Skipped on mainnet for safety" };
    }

    console.log("\n============================================================");
    console.log("GAS ESTIMATION TESTING");
    console.log("============================================================");

    // Test 8: Gas Estimation for Key Functions
    try {
        const [owner] = await ethers.getSigners();
        
        // Estimate gas for emergency pause
        const pauseGasEstimate = await contract.connect(owner).emergencyPause.estimateGas("Gas test");
        console.log("✅ Emergency pause gas estimate:", pauseGasEstimate.toString());
        
        // Estimate gas for bundle mode change
        const bundleGasEstimate = await contract.connect(owner).setBundleOnlyMode.estimateGas(false);
        console.log("✅ Bundle mode change gas estimate:", bundleGasEstimate.toString());
        
        testResults.tests.gasEstimation = { 
            status: "PASS", 
            message: `Pause: ${pauseGasEstimate}, Bundle: ${bundleGasEstimate}` 
        };
    } catch (error) {
        console.log("❌ Gas estimation test failed:", error.message);
        testResults.tests.gasEstimation = { status: "FAIL", message: error.message };
    }

    console.log("\n============================================================");
    console.log("EVENT EMISSION TESTING");
    console.log("============================================================");

    // Test 9: Event Emission
    try {
        const [owner] = await ethers.getSigners();
        
        // Test emergency pause event
        const pauseTx = await contract.connect(owner).emergencyPause("Event test");
        const pauseReceipt = await pauseTx.wait();
        
        const pauseEvent = pauseReceipt.logs.find(log => {
            try {
                const parsed = contract.interface.parseLog(log);
                return parsed.name === "EmergencyPause";
            } catch {
                return false;
            }
        });
        
        if (!pauseEvent) {
            throw new Error("EmergencyPause event not emitted");
        }
        
        console.log("✅ EmergencyPause event emitted");
        testResults.tests.eventEmission = { status: "PASS", message: "Events emitted correctly" };
    } catch (error) {
        console.log("❌ Event emission test failed:", error.message);
        testResults.tests.eventEmission = { status: "FAIL", message: error.message };
    }

    // Calculate overall test results
    const totalTests = Object.keys(testResults.tests).length;
    const passedTests = Object.values(testResults.tests).filter(test => test.status === "PASS").length;
    const failedTests = Object.values(testResults.tests).filter(test => test.status === "FAIL").length;
    const skippedTests = Object.values(testResults.tests).filter(test => test.status === "SKIP").length;

    testResults.summary = {
        total: totalTests,
        passed: passedTests,
        failed: failedTests,
        skipped: skippedTests,
        successRate: (passedTests / (totalTests - skippedTests)) * 100
    };

    // Save test results
    const testResultsFile = path.join(__dirname, "..", "deployments", `test-results-${hre.network.name}.json`);
    fs.writeFileSync(testResultsFile, JSON.stringify(testResults, null, 2));

    // Final summary
    console.log("\n============================================================");
    console.log("TEST RESULTS SUMMARY");
    console.log("============================================================");
    console.log(`Total Tests: ${totalTests}`);
    console.log(`Passed: ${passedTests}`);
    console.log(`Failed: ${failedTests}`);
    console.log(`Skipped: ${skippedTests}`);
    console.log(`Success Rate: ${testResults.summary.successRate.toFixed(1)}%`);
    console.log("============================================================");

    if (failedTests > 0) {
        console.log("❌ Some tests failed. Check the detailed results above.");
        console.log("Test results saved to:", testResultsFile);
        process.exit(1);
    } else {
        console.log("✅ All tests passed!");
        console.log("Test results saved to:", testResultsFile);
    }

    return testResults;
}

// Handle errors gracefully
main()
    .then(() => process.exit(0))
    .catch((error) => {
        console.error("❌ Testing failed:", error);
        process.exit(1);
    });
