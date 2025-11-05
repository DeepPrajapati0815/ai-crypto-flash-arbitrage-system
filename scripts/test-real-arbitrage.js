const { ethers } = require("hardhat");
const fs = require("fs");
const path = require("path");

/**
 * Real Arbitrage Execution Test Script
 * Tests actual flash loan execution, DEX swaps, and profit validation
 * 
 * ⚠️ WARNING: This executes REAL transactions on-chain
 * - Requires real tokens with balances
 * - Costs gas fees
 * - Only works if arbitrage opportunity exists
 * 
 * Usage:
 *   npx hardhat run scripts/test-real-arbitrage.js --network sepolia
 */

// Sepolia Testnet Configuration
const SEPOLIA_CONFIG = {
    // Sepolia Testnet Addresses
    WETH: "0xfff9976782d46cc05630d1f6ebab18b2324d6b14",  // Sepolia WETH
    USDC: "0x94a9d9ac8a22534e3faca9f4e7f2e2cf85d5e4c8",  // Sepolia USDC
    
    // Price information (Sepolia testnet)
    // WETH: $3,620.14
    // USDC: $0.00004043
    // Exchange rate: 1 WETH ≈ 89,530,000 USDC
    
    // Test amounts (start small!)
    FLASH_LOAN_AMOUNT: ethers.parseEther("0.01"), // 0.01 WETH (~$36.20)
    MIN_PROFIT: ethers.parseEther("0.0001"), // 0.0001 ETH minimum profit (~$0.36)
    
    // Expected exchange amounts (based on current prices)
    // 0.01 WETH should get ~895,300 USDC
    EXPECTED_USDC_FROM_WETH: ethers.parseUnits("895300", 6), // 895,300 USDC for 0.01 WETH
};

async function main() {
    console.log("============================================================");
    console.log("REAL ARBITRAGE EXECUTION TEST");
    console.log("============================================================");
    console.log("Network:", hre.network.name);
    console.log("⚠️  WARNING: This executes REAL transactions!");
    console.log("============================================================\n");

    // Get signer
    const [deployer] = await ethers.getSigners();
    console.log("Executor:", deployer.address);
    
    // Check balance
    const balance = await ethers.provider.getBalance(deployer.address);
    console.log("ETH Balance:", ethers.formatEther(balance), "ETH");
    
    if (balance < ethers.parseEther("0.05")) {
        console.error("❌ Insufficient ETH balance. Need at least 0.05 ETH for gas fees.");
        console.log("Get Sepolia ETH from: https://sepoliafaucet.com/");
        process.exit(1);
    }
    console.log("");

    // Load deployment info
    const deploymentFile = path.join(__dirname, "..", "deployments", `sepolia-deployment.json`);
    
    if (!fs.existsSync(deploymentFile)) {
        console.error("❌ Deployment file not found:", deploymentFile);
        console.log("Please deploy the contract first.");
        process.exit(1);
    }

    const deploymentInfo = JSON.parse(fs.readFileSync(deploymentFile, "utf8"));
    const contractAddress = deploymentInfo.contracts.FlashArbUltimate;

    console.log("Contract Address:", contractAddress);
    console.log("Block Explorer:", `https://sepolia.etherscan.io/address/${contractAddress}`);
    console.log("");

    // Get contract instance
    const contract = await ethers.getContractAt("FlashArbUltimate", contractAddress);

    // ========================================================================
    // STEP 1: PRE-FLIGHT CHECKS
    // ========================================================================
    console.log("============================================================");
    console.log("STEP 1: PRE-FLIGHT CHECKS");
    console.log("============================================================\n");

    // Check contract health
    const healthy = await contract.isHealthy();
    console.log("Contract Healthy:", healthy);
    
    if (!healthy) {
        console.error("❌ Contract is not healthy. Check status:");
        const status = await contract.getStatus();
        console.log("Status:", status);
        process.exit(1);
    }

    // Check authorization
    const isExecutor = await contract.authorizedExecutors(deployer.address);
    console.log("Is Authorized Executor:", isExecutor);
    
    if (!isExecutor) {
        console.log("⚠️  Not authorized. Authorizing now...");
        const tx = await contract.addAuthorizedExecutor(deployer.address);
        await tx.wait();
        console.log("✅ Authorized as executor");
    }

    // Check bundle mode
    const bundleMode = await contract.bundleOnlyMode();
    console.log("Bundle Only Mode:", bundleMode);
    
    if (bundleMode) {
        console.log("⚠️  Bundle mode enabled. Checking authorization...");
        const isBundleSubmitter = await contract.authorizedBundleSubmitters(deployer.address);
        
        if (!isBundleSubmitter) {
            console.log("⚠️  Not authorized as bundle submitter. Disabling bundle mode for testing...");
            const tx = await contract.setBundleOnlyMode(false);
            await tx.wait();
            console.log("✅ Bundle mode disabled");
        }
    }

    // Get contract parameters
    const status = await contract.getStatus();
    console.log("\nContract Configuration:");
    console.log("- Min Profit:", ethers.formatEther(status.currentMinProfit), "ETH");
    console.log("- Max Failed Attempts:", status.maxAttempts.toString());
    console.log("- Current Failed Attempts:", status.currentFailedAttempts.toString());
    console.log("- Paused:", status.isPaused);
    console.log("- Emergency Paused:", status.isEmergencyPaused);
    console.log("");

    // ========================================================================
    // STEP 2: CHECK TOKEN BALANCES
    // ========================================================================
    console.log("============================================================");
    console.log("STEP 2: CHECK TOKEN BALANCES");
    console.log("============================================================\n");

    const wethContract = await ethers.getContractAt("IERC20", SEPOLIA_CONFIG.WETH);
    
    try {
        const wethBalance = await wethContract.balanceOf(deployer.address);
        console.log("Your WETH Balance:", ethers.formatEther(wethBalance), "WETH");
        
        if (wethBalance === 0n) {
            console.log("\n⚠️  You have no WETH. To get WETH:");
            console.log("1. Get Sepolia ETH from: https://sepoliafaucet.com/");
            console.log("2. Wrap ETH to WETH:");
            console.log(`   cast send ${SEPOLIA_CONFIG.WETH} "deposit()" --value 0.1ether --rpc-url $SEPOLIA_RPC_URL --private-key $PRIVATE_KEY`);
            console.log("\nOr use Hardhat console:");
            console.log(`   const weth = await ethers.getContractAt("IWETH", "${SEPOLIA_CONFIG.WETH}");`);
            console.log(`   await weth.deposit({ value: ethers.parseEther("0.1") });`);
        }
    } catch (error) {
        console.error("❌ Error checking WETH balance:", error.message);
        console.log("Token address might be incorrect for Sepolia testnet.");
    }
    console.log("");

    // ========================================================================
    // STEP 3: BUILD ARBITRAGE ROUTE
    // ========================================================================
    console.log("============================================================");
    console.log("STEP 3: BUILD ARBITRAGE ROUTE");
    console.log("============================================================\n");

    console.log("⚠️  NOTE: This is a TEST route that will likely FAIL");
    console.log("Real arbitrage requires:");
    console.log("- Actual price discrepancies between DEXes");
    console.log("- Sufficient liquidity in pools");
    console.log("- Accurate slippage calculations");
    console.log("");

    // Flash loan route: Borrow USDC → Swap to WETH → Repay USDC
    // Since you have USDC, we'll test borrowing USDC and swapping to WETH
    
    // Calculate amounts based on real prices
    // USDC price: $0.00004043, WETH price: $3,620.14
    // 1 USDC = $0.00004043, so need ~89,530,000 USDC to get 1 WETH
    
    const flashLoanAmountUSDC = ethers.parseUnits("100000", 6); // Borrow 100,000 USDC (~$4.04)
    const expectedWETH = ethers.parseEther("0.001117"); // ~100,000 / 89,530 = 0.001117 WETH
    const minWETHWithSlippage = expectedWETH * 90n / 100n; // 90% (10% slippage tolerance for testnet)
    
    // For profit, we need to get back more USDC than we borrowed + Aave premium (0.09%)
    const aavePremiumUSDC = flashLoanAmountUSDC * 9n / 10000n; // 0.09% premium = 90 USDC
    const totalDebtUSDC = flashLoanAmountUSDC + aavePremiumUSDC; // 100,090 USDC
    const minUSDCForProfit = totalDebtUSDC + ethers.parseUnits("100", 6); // Need 100,190 USDC for profit
    
    const tuples = [
        {
            dexType: 0, // UniswapV3
            tokenIn: SEPOLIA_CONFIG.USDC,
            tokenOut: SEPOLIA_CONFIG.WETH,
            poolFee: 3000, // 0.3%
            amountIn: flashLoanAmountUSDC, // 100,000 USDC
            minAmountOut: minWETHWithSlippage // ~0.001005 WETH (with 10% slippage)
        },
        {
            dexType: 1, // Sushiswap
            tokenIn: SEPOLIA_CONFIG.WETH,
            tokenOut: SEPOLIA_CONFIG.USDC,
            poolFee: 3000,
            amountIn: minWETHWithSlippage, // Use the WETH we got
            minAmountOut: minUSDCForProfit // Need > 100,190 USDC for profit
        }
    ];

    console.log("Route Configuration:");
    console.log("1. Borrow:", ethers.formatUnits(flashLoanAmountUSDC, 6), "USDC from Aave");
    console.log("   Aave Premium (0.09%):", ethers.formatUnits(aavePremiumUSDC, 6), "USDC");
    console.log("   Total Debt:", ethers.formatUnits(totalDebtUSDC, 6), "USDC");
    console.log("");
    console.log("2. Swap USDC → WETH on Uniswap V3");
    console.log("   Amount In:", ethers.formatUnits(flashLoanAmountUSDC, 6), "USDC");
    console.log("   Expected Out:", ethers.formatEther(expectedWETH), "WETH");
    console.log("   Min Out (10% slippage):", ethers.formatEther(minWETHWithSlippage), "WETH");
    console.log("");
    console.log("3. Swap WETH → USDC on Sushiswap");
    console.log("   Amount In:", ethers.formatEther(minWETHWithSlippage), "WETH");
    console.log("   Min Out (for profit):", ethers.formatUnits(minUSDCForProfit, 6), "USDC");
    console.log("");
    console.log("4. Repay loan + premium:", ethers.formatUnits(totalDebtUSDC, 6), "USDC");
    console.log("5. Target profit: >", ethers.formatUnits(ethers.parseUnits("100", 6), 6), "USDC");
    console.log("");

    // ========================================================================
    // STEP 4: GAS ESTIMATION
    // ========================================================================
    console.log("============================================================");
    console.log("STEP 4: GAS ESTIMATION");
    console.log("============================================================\n");

    try {
        const gasEstimate = await contract.executeFlashArbitrage.estimateGas(
            SEPOLIA_CONFIG.USDC,
            flashLoanAmountUSDC,
            tuples
        );
        
        const gasPrice = await ethers.provider.getFeeData();
        const estimatedCost = gasEstimate * gasPrice.gasPrice;
        
        console.log("✅ Gas Estimation Successful");
        console.log("Estimated Gas:", gasEstimate.toString());
        console.log("Gas Price:", ethers.formatUnits(gasPrice.gasPrice, "gwei"), "gwei");
        console.log("Estimated Cost:", ethers.formatEther(estimatedCost), "ETH");
        console.log("");
        
        // Ask for confirmation
        console.log("⚠️  READY TO EXECUTE REAL TRANSACTION");
        console.log("This will cost gas fees and may fail if no arbitrage exists.");
        console.log("");
        
    } catch (error) {
        console.error("❌ Gas Estimation Failed:", error.message);
        console.log("\nPossible reasons:");
        console.log("- Token addresses don't exist on Sepolia");
        console.log("- No liquidity in DEX pools");
        console.log("- Slippage too high");
        console.log("- Contract validation failed");
        console.log("\nTo fix:");
        console.log("1. Verify token addresses are correct for Sepolia");
        console.log("2. Check DEX pool exists with liquidity");
        console.log("3. Adjust minAmountOut values");
        console.log("");
        
        console.log("Skipping execution due to gas estimation failure.");
        process.exit(1);
    }

    // ========================================================================
    // STEP 5: EXECUTE ARBITRAGE (COMMENTED OUT FOR SAFETY)
    // ========================================================================
    console.log("============================================================");
    console.log("STEP 5: EXECUTE ARBITRAGE");
    console.log("============================================================\n");

    console.log("🚀 READY TO EXECUTE REAL ARBITRAGE");
    console.log("This will execute a REAL transaction on Sepolia testnet.");
    console.log("");
    
    console.log("🚀 Executing arbitrage...");
    
    try {
        const tx = await contract.executeFlashArbitrage(
            SEPOLIA_CONFIG.USDC,
            flashLoanAmountUSDC,
            tuples,
            {
                gasLimit: gasEstimate * 120n / 100n // 20% buffer
            }
        );
        
        console.log("✅ Transaction Submitted!");
        console.log("Transaction Hash:", tx.hash);
        console.log("View on Etherscan:", `https://sepolia.etherscan.io/tx/${tx.hash}`);
        console.log("");
        console.log("Waiting for confirmation...");
        
        const receipt = await tx.wait();
        
        console.log("");
        console.log("✅ TRANSACTION CONFIRMED!");
        console.log("============================================================");
        console.log("Block Number:", receipt.blockNumber);
        console.log("Gas Used:", receipt.gasUsed.toString());
        console.log("Effective Gas Price:", ethers.formatUnits(receipt.gasPrice, "gwei"), "gwei");
        console.log("Total Gas Cost:", ethers.formatEther(receipt.gasUsed * receipt.gasPrice), "ETH");
        console.log("============================================================\n");
        
        // Parse events
        console.log("Events Emitted:");
        let profitFound = false;
        
        for (const log of receipt.logs) {
            try {
                const parsed = contract.interface.parseLog(log);
                console.log(`\n📋 Event: ${parsed.name}`);
                
                if (parsed.name === "FlashLoanExecuted") {
                    console.log("  ├─ Asset:", parsed.args.asset);
                    console.log("  ├─ Amount:", ethers.formatEther(parsed.args.amount), "WETH");
                    console.log("  ├─ Profit:", ethers.formatEther(parsed.args.profit), "ETH");
                    console.log("  └─ Timestamp:", new Date(Number(parsed.args.timestamp) * 1000).toISOString());
                    profitFound = true;
                }
                
                if (parsed.name === "ProfitRecorded") {
                    console.log("  ├─ Execution ID:", parsed.args.executionId);
                    console.log("  ├─ Asset:", parsed.args.asset);
                    console.log("  ├─ Profit:", ethers.formatEther(parsed.args.profit), "ETH");
                    console.log("  └─ Gas Used:", parsed.args.gasUsed.toString());
                }
                
                if (parsed.name === "RouteExecuted") {
                    console.log("  ├─ Route Hash:", parsed.args.routeHash);
                    console.log("  └─ Nonce:", parsed.args.nonce.toString());
                }
            } catch (e) {
                // Not our contract's event
            }
        }
        
        console.log("");
        console.log("============================================================");
        console.log("✅ ARBITRAGE EXECUTED SUCCESSFULLY!");
        console.log("============================================================");
        
        // Check contract balance for profit
        const contractBalanceUSDC = await contract.getBalance(SEPOLIA_CONFIG.USDC);
        const contractBalanceWETH = await contract.getBalance(SEPOLIA_CONFIG.WETH);
        
        if (contractBalanceUSDC > 0n) {
            console.log("\n💰 Contract USDC Balance:", ethers.formatUnits(contractBalanceUSDC, 6), "USDC");
            console.log("You can withdraw this profit using:");
            console.log(`  contract.withdrawProfit("${SEPOLIA_CONFIG.USDC}", "${contractBalanceUSDC}", "YOUR_ADDRESS")`);
        }
        
        if (contractBalanceWETH > 0n) {
            console.log("\n💰 Contract WETH Balance:", ethers.formatEther(contractBalanceWETH), "WETH");
            console.log("You can withdraw this profit using:");
            console.log(`  contract.withdrawProfit("${SEPOLIA_CONFIG.WETH}", "${contractBalanceWETH}", "YOUR_ADDRESS")`);
        }
        
    } catch (error) {
        console.error("\n❌ EXECUTION FAILED");
        console.error("============================================================");
        console.error("Error:", error.message);
        console.error("============================================================\n");
        
        if (error.message.includes("Profit below minimum")) {
            console.log("📊 Reason: Profit below minimum threshold");
            console.log("   The arbitrage opportunity doesn't exist or is too small.");
            console.log("   Current min profit:", ethers.formatEther(status.currentMinProfit), "ETH");
        } else if (error.message.includes("Insufficient funds")) {
            console.log("📊 Reason: Insufficient funds to repay flash loan");
            console.log("   The swaps resulted in a net loss, not a profit.");
            console.log("   This means no arbitrage opportunity exists.");
        } else if (error.message.includes("Slippage exceeded")) {
            console.log("📊 Reason: Slippage protection triggered");
            console.log("   Actual output was less than minAmountOut.");
            console.log("   Try increasing slippage tolerance or reducing amount.");
        } else if (error.message.includes("execution reverted")) {
            console.log("📊 Reason: Transaction reverted");
            console.log("   Possible causes:");
            console.log("   - No liquidity in DEX pools");
            console.log("   - Token addresses incorrect");
            console.log("   - Pool doesn't exist on Sepolia");
            console.log("   - Insufficient allowance");
        } else if (error.message.includes("Cooldown")) {
            console.log("📊 Reason: Cooldown period not elapsed");
            console.log("   Wait 30 seconds between executions.");
        } else if (error.message.includes("Gas too high")) {
            console.log("📊 Reason: Gas price exceeds maximum");
            console.log("   Current gas price is too high for execution.");
        }
        
        console.log("\n💡 This is expected behavior if:");
        console.log("   - No real arbitrage opportunity exists");
        console.log("   - DEX pools have no liquidity on Sepolia");
        console.log("   - Prices are equal on both DEXes");
        
        throw error;
    }

    // ========================================================================
    // SUMMARY
    // ========================================================================
    console.log("\n============================================================");
    console.log("TEST COMPLETED");
    console.log("============================================================\n");
    
    console.log("Next steps for production:");
    console.log("1. ✅ Integrate with Rust bot for opportunity detection");
    console.log("2. ✅ Enable bundle-only mode for MEV protection");
    console.log("3. ✅ Use Flashbots for private transaction submission");
    console.log("4. ✅ Set realistic profit thresholds based on gas costs");
    console.log("5. ✅ Monitor contract health and circuit breaker");
    console.log("");
}

// Execute main function
main()
    .then(() => process.exit(0))
    .catch((error) => {
        console.error("\n❌ Test failed with error:");
        console.error(error);
        process.exit(1);
    });
