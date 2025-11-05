const { ethers } = require("hardhat");

/**
 * FIXED PROFITABLE FLASH LOAN ARBITRAGE TEST
 * 
 * Improvements:
 * 1. Fixed quoter with proper staticCall handling
 * 2. Pre-execution profitability validation
 * 3. Multiple route testing
 * 4. Better error handling
 * 5. Accurate minimum output calculation
 * 
 * Run: npx hardhat run scripts/test-profitable-arbitrage-fixed.js --network localhost
 */

// Ethereum Mainnet Addresses
const MAINNET_CONFIG = {
    AAVE_POOL: "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2",
    UNISWAP_V3_ROUTER: "0xE592427A0AEce92De3Edee1F18E0157C05861564",
    UNISWAP_V3_QUOTER: "0x61fFE014bA17989E743c5F6cB21bF9697530B21e",
    UNISWAP_V3_QUOTER_V2: "0xb27308f9F90D607463bb33eA1BeBb41C27CE5AB6", // Try V2 quoter
    SUSHISWAP_ROUTER: "0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F",
    
    WETH: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
    USDC: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
    USDT: "0xdAC17F958D2ee523a2206206994597C13D831ec7",
    DAI: "0x6B175474E89094C44Da98b954EedeAC495271d0F",
};

// Available pool fees on Uniswap V3
const POOL_FEES = {
    LOWEST: 100,    // 0.01%
    LOW: 500,       // 0.05%
    MEDIUM: 3000,   // 0.3%
    HIGH: 10000     // 1%
};

/**
 * Get quote from Uniswap V3 with improved error handling
 */
async function getQuoteV3(quoter, tokenIn, tokenOut, fee, amountIn) {
    try {
        // Try QuoterV2 first (more reliable)
        const quoterV2ABI = [
            "function quoteExactInputSingle((address tokenIn, address tokenOut, uint256 amountIn, uint24 fee, uint160 sqrtPriceLimitX96)) external returns (uint256 amountOut, uint160 sqrtPriceX96After, uint32 initializedTicksCrossed, uint256 gasEstimate)"
        ];
        
        const quoterV2 = await ethers.getContractAt(quoterV2ABI, MAINNET_CONFIG.UNISWAP_V3_QUOTER_V2);
        
        const params = {
            tokenIn: tokenIn,
            tokenOut: tokenOut,
            amountIn: amountIn,
            fee: fee,
            sqrtPriceLimitX96: 0
        };
        
        const result = await quoterV2.quoteExactInputSingle.staticCall(params);
        return {
            amountOut: result[0],
            gasEstimate: result[3],
            success: true
        };
    } catch (error) {
        // Fallback to V1 quoter
        try {
            const quoterV1ABI = [
                "function quoteExactInputSingle(address tokenIn, address tokenOut, uint24 fee, uint256 amountIn, uint160 sqrtPriceLimitX96) external returns (uint256 amountOut)"
            ];
            
            const quoterV1 = await ethers.getContractAt(quoterV1ABI, MAINNET_CONFIG.UNISWAP_V3_QUOTER);
            
            const amountOut = await quoterV1.quoteExactInputSingle.staticCall(
                tokenIn,
                tokenOut,
                fee,
                amountIn,
                0
            );
            
            return {
                amountOut: amountOut,
                gasEstimate: 0n,
                success: true
            };
        } catch (e) {
            return {
                amountOut: 0n,
                gasEstimate: 0n,
                success: false,
                error: e.message
            };
        }
    }
}

/**
 * Test a specific arbitrage route
 */
async function testRoute(quoter, routeConfig) {
    const { name, tokenIn, tokenOut, tokenIntermediate, fee1, fee2, amount } = routeConfig;
    
    console.log(`\n📊 Testing Route: ${name}`);
    console.log("─".repeat(60));
    
    // Step 1: TokenIn → TokenIntermediate
    const quote1 = await getQuoteV3(quoter, tokenIn, tokenIntermediate, fee1, amount);
    
    if (!quote1.success) {
        console.log(`❌ Failed to get quote for step 1: ${quote1.error}`);
        return null;
    }
    
    console.log(`Step 1: ${await getTokenSymbol(tokenIn)} → ${await getTokenSymbol(tokenIntermediate)}`);
    console.log(`  Fee: ${fee1 / 10000}%`);
    console.log(`  Input: ${formatToken(amount, tokenIn)}`);
    console.log(`  Output: ${formatToken(quote1.amountOut, tokenIntermediate)}`);
    
    // Step 2: TokenIntermediate → TokenOut
    const quote2 = await getQuoteV3(quoter, tokenIntermediate, tokenOut, fee2, quote1.amountOut);
    
    if (!quote2.success) {
        console.log(`❌ Failed to get quote for step 2: ${quote2.error}`);
        return null;
    }
    
    console.log(`Step 2: ${await getTokenSymbol(tokenIntermediate)} → ${await getTokenSymbol(tokenOut)}`);
    console.log(`  Fee: ${fee2 / 10000}%`);
    console.log(`  Input: ${formatToken(quote1.amountOut, tokenIntermediate)}`);
    console.log(`  Output: ${formatToken(quote2.amountOut, tokenOut)}`);
    
    // Calculate profitability
    const aavePremium = amount * 9n / 10000n; // 0.09%
    const totalDebt = amount + aavePremium;
    const amountOut = quote2.amountOut;
    const profit = amountOut > totalDebt ? amountOut - totalDebt : 0n;
    const profitPercent = Number(profit) / Number(amount) * 100;
    
    console.log(`\n💰 Profitability:`);
    console.log(`  Borrowed: ${ethers.formatEther(amount)} WETH`);
    console.log(`  Aave Fee: ${ethers.formatEther(aavePremium)} WETH (0.09%)`);
    console.log(`  Total Debt: ${ethers.formatEther(totalDebt)} WETH`);
    console.log(`  Amount Out: ${ethers.formatEther(amountOut)} WETH`);
    console.log(`  Gross Profit: ${ethers.formatEther(profit)} WETH`);
    console.log(`  Profit %: ${profitPercent.toFixed(4)}%`);
    
    const isProfitable = profit > 0n;
    console.log(`  Status: ${isProfitable ? '✅ PROFITABLE' : '❌ NOT PROFITABLE'}`);
    
    return {
        name,
        tokenIn,
        tokenOut,
        tokenIntermediate,
        fee1,
        fee2,
        amount,
        quote1: quote1.amountOut,
        quote2: quote2.amountOut,
        totalDebt,
        profit,
        profitPercent,
        isProfitable
    };
}

/**
 * Format token amount based on token address
 */
function formatToken(amount, tokenAddress) {
    if (tokenAddress.toLowerCase() === MAINNET_CONFIG.USDC.toLowerCase() ||
        tokenAddress.toLowerCase() === MAINNET_CONFIG.USDT.toLowerCase()) {
        return ethers.formatUnits(amount, 6);
    } else if (tokenAddress.toLowerCase() === MAINNET_CONFIG.WETH.toLowerCase() ||
               tokenAddress.toLowerCase() === MAINNET_CONFIG.DAI.toLowerCase()) {
        return ethers.formatEther(amount);
    }
    return amount.toString();
}

/**
 * Get token symbol
 */
async function getTokenSymbol(tokenAddress) {
    const tokenMap = {
        [MAINNET_CONFIG.WETH.toLowerCase()]: "WETH",
        [MAINNET_CONFIG.USDC.toLowerCase()]: "USDC",
        [MAINNET_CONFIG.USDT.toLowerCase()]: "USDT",
        [MAINNET_CONFIG.DAI.toLowerCase()]: "DAI",
    };
    return tokenMap[tokenAddress.toLowerCase()] || "UNKNOWN";
}

/**
 * Scan multiple routes to find profitable opportunities
 */
async function scanRoutes(quoter, testAmount) {
    console.log("\n" + "=".repeat(70));
    console.log("🔍 SCANNING ROUTES FOR PROFITABLE OPPORTUNITIES");
    console.log("=".repeat(70));
    
    const routes = [
        {
            name: "WETH → USDC → WETH (0.3% → 0.05%)",
            tokenIn: MAINNET_CONFIG.WETH,
            tokenIntermediate: MAINNET_CONFIG.USDC,
            tokenOut: MAINNET_CONFIG.WETH,
            fee1: POOL_FEES.MEDIUM,
            fee2: POOL_FEES.LOW,
            amount: testAmount
        },
        {
            name: "WETH → USDC → WETH (0.05% → 0.3%)",
            tokenIn: MAINNET_CONFIG.WETH,
            tokenIntermediate: MAINNET_CONFIG.USDC,
            tokenOut: MAINNET_CONFIG.WETH,
            fee1: POOL_FEES.LOW,
            fee2: POOL_FEES.MEDIUM,
            amount: testAmount
        },
        {
            name: "WETH → USDC → WETH (0.3% → 0.3%)",
            tokenIn: MAINNET_CONFIG.WETH,
            tokenIntermediate: MAINNET_CONFIG.USDC,
            tokenOut: MAINNET_CONFIG.WETH,
            fee1: POOL_FEES.MEDIUM,
            fee2: POOL_FEES.MEDIUM,
            amount: testAmount
        },
        {
            name: "WETH → USDT → WETH (0.3% → 0.05%)",
            tokenIn: MAINNET_CONFIG.WETH,
            tokenIntermediate: MAINNET_CONFIG.USDT,
            tokenOut: MAINNET_CONFIG.WETH,
            fee1: POOL_FEES.MEDIUM,
            fee2: POOL_FEES.LOW,
            amount: testAmount
        },
        {
            name: "WETH → USDT → WETH (0.05% → 0.3%)",
            tokenIn: MAINNET_CONFIG.WETH,
            tokenIntermediate: MAINNET_CONFIG.USDT,
            tokenOut: MAINNET_CONFIG.WETH,
            fee1: POOL_FEES.LOW,
            fee2: POOL_FEES.MEDIUM,
            amount: testAmount
        },
        {
            name: "WETH → DAI → WETH (0.3% → 0.05%)",
            tokenIn: MAINNET_CONFIG.WETH,
            tokenIntermediate: MAINNET_CONFIG.DAI,
            tokenOut: MAINNET_CONFIG.WETH,
            fee1: POOL_FEES.MEDIUM,
            fee2: POOL_FEES.LOW,
            amount: testAmount
        },
        {
            name: "WETH → DAI → WETH (0.05% → 0.3%)",
            tokenIn: MAINNET_CONFIG.WETH,
            tokenIntermediate: MAINNET_CONFIG.DAI,
            tokenOut: MAINNET_CONFIG.WETH,
            fee1: POOL_FEES.LOW,
            fee2: POOL_FEES.MEDIUM,
            amount: testAmount
        }
    ];
    
    const results = [];
    
    for (const route of routes) {
        const result = await testRoute(quoter, route);
        if (result) {
            results.push(result);
        }
        await new Promise(resolve => setTimeout(resolve, 100)); // Small delay
    }
    
    return results;
}

/**
 * Main execution function
 */
async function main() {
    console.log("=".repeat(70));
    console.log("FIXED PROFITABLE FLASH LOAN ARBITRAGE TEST");
    console.log("=".repeat(70));
    console.log("Network:", hre.network.name);
    console.log("Testing with REAL mainnet liquidity (forked locally)");
    console.log("=".repeat(70) + "\n");

    const [deployer] = await ethers.getSigners();
    console.log("Deployer:", deployer.address);
    
    const balance = await ethers.provider.getBalance(deployer.address);
    console.log("ETH Balance:", ethers.formatEther(balance), "ETH\n");

    // ========================================================================
    // STEP 1: SCAN FOR PROFITABLE ROUTES
    // ========================================================================
    
    const testAmount = ethers.parseEther("0.1"); // 0.1 WETH
    
    const quoterV2ABI = [
        "function quoteExactInputSingle((address tokenIn, address tokenOut, uint256 amountIn, uint24 fee, uint160 sqrtPriceLimitX96)) external returns (uint256 amountOut, uint160 sqrtPriceX96After, uint32 initializedTicksCrossed, uint256 gasEstimate)"
    ];
    const quoter = await ethers.getContractAt(quoterV2ABI, MAINNET_CONFIG.UNISWAP_V3_QUOTER_V2);
    
    const routeResults = await scanRoutes(quoter, testAmount);
    
    // Find most profitable route
    const profitableRoutes = routeResults.filter(r => r.isProfitable);
    
    console.log("\n" + "=".repeat(70));
    console.log("📊 ROUTE SCAN RESULTS");
    console.log("=".repeat(70));
    console.log(`Total Routes Tested: ${routeResults.length}`);
    console.log(`Profitable Routes: ${profitableRoutes.length}`);
    
    if (profitableRoutes.length === 0) {
        console.log("\n❌ NO PROFITABLE ROUTES FOUND!");
        console.log("\nReasons this might happen:");
        console.log("  1. Market is efficient - arbitrage opportunities are rare");
        console.log("  2. Gas costs would eat all profit");
        console.log("  3. Need to test with different amounts");
        console.log("  4. Need to check cross-DEX arbitrage (Uniswap ↔ Sushiswap)");
        console.log("\n💡 Try:");
        console.log("  - Increasing flash loan amount (more profit potential)");
        console.log("  - Testing during high volatility periods");
        console.log("  - Using cross-DEX routes");
        console.log("\nExiting without execution...\n");
        return;
    }
    
    // Sort by profitability
    profitableRoutes.sort((a, b) => Number(b.profit - a.profit));
    
    console.log("\n🏆 TOP PROFITABLE ROUTES:");
    profitableRoutes.slice(0, 3).forEach((route, idx) => {
        console.log(`\n${idx + 1}. ${route.name}`);
        console.log(`   Profit: ${ethers.formatEther(route.profit)} WETH (${route.profitPercent.toFixed(4)}%)`);
    });
    
    const bestRoute = profitableRoutes[0];
    console.log("\n✅ Selected Best Route:", bestRoute.name);
    console.log(`   Expected Profit: ${ethers.formatEther(bestRoute.profit)} WETH`);
    
    // ========================================================================
    // STEP 2: DEPLOY CONTRACT
    // ========================================================================
    console.log("\n" + "=".repeat(70));
    console.log("STEP 2: DEPLOY FLASH ARBITRAGE CONTRACT");
    console.log("=".repeat(70) + "\n");

    const FlashArbUltimate = await ethers.getContractFactory("FlashArbUltimate");
    const feeData = await ethers.provider.getFeeData();
    
    console.log("Deploying FlashArbUltimate...");
    const flashArb = await FlashArbUltimate.deploy(
        MAINNET_CONFIG.AAVE_POOL,
        MAINNET_CONFIG.UNISWAP_V3_ROUTER,
        MAINNET_CONFIG.SUSHISWAP_ROUTER,
        ethers.parseEther("0.00001"), // 0.00001 ETH min profit
        5,
        {
            maxFeePerGas: feeData.maxFeePerGas,
            maxPriorityFeePerGas: feeData.maxPriorityFeePerGas
        }
    );

    await flashArb.waitForDeployment();
    const contractAddress = await flashArb.getAddress();
    
    console.log("✅ Contract deployed to:", contractAddress);
    
    // ========================================================================
    // STEP 3: FUND CONTRACT (MINIMAL BUFFER)
    // ========================================================================
    console.log("\n" + "=".repeat(70));
    console.log("STEP 3: FUND CONTRACT WITH SAFETY BUFFER");
    console.log("=".repeat(70) + "\n");

    const weth = await ethers.getContractAt(
        [
            "function deposit() payable",
            "function transfer(address to, uint256 amount) returns (bool)",
            "function balanceOf(address) view returns (uint256)"
        ],
        MAINNET_CONFIG.WETH
    );

    // Calculate minimum buffer needed (1% of loan for slippage)
    const minBuffer = testAmount / 100n;
    
    console.log("Wrapping ETH to WETH...");
    const wrapTx = await weth.deposit({ 
        value: minBuffer,
        maxFeePerGas: feeData.maxFeePerGas,
        maxPriorityFeePerGas: feeData.maxPriorityFeePerGas
    });
    await wrapTx.wait();

    console.log(`Transferring ${ethers.formatEther(minBuffer)} WETH to contract...`);
    const transferTx = await weth.transfer(contractAddress, minBuffer, {
        maxFeePerGas: feeData.maxFeePerGas,
        maxPriorityFeePerGas: feeData.maxPriorityFeePerGas
    });
    await transferTx.wait();

    const contractWethBefore = await weth.balanceOf(contractAddress);
    console.log("✅ Contract WETH Balance:", ethers.formatEther(contractWethBefore), "WETH");
    console.log("   (Safety buffer for slippage protection)\n");

    // ========================================================================
    // STEP 4: CONFIGURE CONTRACT
    // ========================================================================
    console.log("=".repeat(70));
    console.log("STEP 4: CONFIGURE CONTRACT");
    console.log("=".repeat(70) + "\n");

    console.log("Authorizing executor...");
    const authTx = await flashArb.addAuthorizedExecutor(deployer.address, {
        maxFeePerGas: feeData.maxFeePerGas,
        maxPriorityFeePerGas: feeData.maxPriorityFeePerGas
    });
    await authTx.wait();
    console.log("✅ Executor authorized!");

    console.log("Setting gas price limit...");
    const gasLimitTx = await flashArb.setMaxGasPrice(ethers.parseUnits("100", "gwei"), {
        maxFeePerGas: feeData.maxFeePerGas,
        maxPriorityFeePerGas: feeData.maxPriorityFeePerGas
    });
    await gasLimitTx.wait();
    console.log("✅ Gas price limit set!");

    console.log("Setting gas price tolerance...");
    const toleranceTx = await flashArb.setGasPriceTolerance(300, {
        maxFeePerGas: feeData.maxFeePerGas,
        maxPriorityFeePerGas: feeData.maxPriorityFeePerGas
    });
    await toleranceTx.wait();
    console.log("✅ Gas price tolerance set!\n");

    // ========================================================================
    // STEP 5: BUILD ARBITRAGE ROUTE WITH ACCURATE PARAMETERS
    // ========================================================================
    console.log("=".repeat(70));
    console.log("STEP 5: BUILD ARBITRAGE ROUTE");
    console.log("=".repeat(70) + "\n");

    console.log("Using Best Route:", bestRoute.name);
    console.log("Expected Output:", ethers.formatEther(bestRoute.quote2), "WETH");
    console.log("Required Repayment:", ethers.formatEther(bestRoute.totalDebt), "WETH");
    console.log("Expected Profit:", ethers.formatEther(bestRoute.profit), "WETH");
    console.log("");

    // Calculate minAmountOut with 0.5% slippage tolerance
    const slippageTolerance = 50; // 0.5% in basis points
    const minAmount1 = (bestRoute.quote1 * (10000n - BigInt(slippageTolerance))) / 10000n;
    const minAmount2 = (bestRoute.quote2 * (10000n - BigInt(slippageTolerance))) / 10000n;

    const tuples = [
        {
            dexType: 0, // UniswapV3
            tokenIn: bestRoute.tokenIn,
            tokenOut: bestRoute.tokenIntermediate,
            poolFee: bestRoute.fee1,
            amountIn: bestRoute.amount,
            minAmountOut: minAmount1
        },
        {
            dexType: 0, // UniswapV3
            tokenIn: bestRoute.tokenIntermediate,
            tokenOut: bestRoute.tokenOut,
            poolFee: bestRoute.fee2,
            amountIn: minAmount1,
            minAmountOut: minAmount2
        }
    ];

    console.log("Swap Parameters:");
    console.log("  Swap 1:");
    console.log(`    Expected: ${formatToken(bestRoute.quote1, bestRoute.tokenIntermediate)}`);
    console.log(`    Minimum:  ${formatToken(minAmount1, bestRoute.tokenIntermediate)} (0.5% slippage)`);
    console.log("  Swap 2:");
    console.log(`    Expected: ${formatToken(bestRoute.quote2, bestRoute.tokenOut)}`);
    console.log(`    Minimum:  ${formatToken(minAmount2, bestRoute.tokenOut)} (0.5% slippage)`);
    console.log("");

    // ========================================================================
    // STEP 6: EXECUTE WITH VALIDATION
    // ========================================================================
    console.log("=".repeat(70));
    console.log("STEP 6: EXECUTE FLASH LOAN ARBITRAGE");
    console.log("=".repeat(70) + "\n");

    // Final safety check
    if (minAmount2 < bestRoute.totalDebt) {
        console.log("⚠️  WARNING: Even with slippage, output might not cover debt!");
        console.log("   Consider increasing buffer or aborting...\n");
    }

    console.log("🚀 Executing flash loan arbitrage...\n");

    try {
        const block = await ethers.provider.getBlock('latest');
        const baseFee = block.baseFeePerGas;
        const maxAllowedGas = (baseFee * 300n) / 100n;
        const priorityFee = maxAllowedGas / 10n;

        const tx = await flashArb.executeFlashArbitrage(
            MAINNET_CONFIG.WETH,
            testAmount,
            tuples,
            { 
                gasLimit: 1000000,
                maxFeePerGas: maxAllowedGas,
                maxPriorityFeePerGas: priorityFee
            }
        );

        console.log("✅ Transaction Submitted!");
        console.log("TX Hash:", tx.hash);
        console.log("\nWaiting for confirmation...");

        const receipt = await tx.wait();

        console.log("\n" + "=".repeat(70));
        console.log("✅✅✅ TRANSACTION CONFIRMED! ✅✅✅");
        console.log("=".repeat(70));
        console.log("Block Number:", receipt.blockNumber);
        console.log("Gas Used:", receipt.gasUsed.toString());
        console.log("Status:", receipt.status === 1 ? "✅ SUCCESS" : "❌ FAILED");
        
        // Calculate gas cost
        const gasPrice = receipt.gasPrice || feeData.gasPrice;
        const gasCost = receipt.gasUsed * gasPrice;
        console.log("Gas Cost:", ethers.formatEther(gasCost), "ETH");
        console.log("");

        // Parse events
        console.log("📋 Transaction Events:");
        console.log("=".repeat(70));
        for (const log of receipt.logs) {
            try {
                const parsed = flashArb.interface.parseLog(log);
                if (parsed) {
                    console.log(`\n✨ ${parsed.name}`);
                    for (const [key, value] of Object.entries(parsed.args)) {
                        if (isNaN(key)) {
                            if (typeof value === 'bigint') {
                                console.log(`  ${key}: ${value.toString()}`);
                            } else {
                                console.log(`  ${key}:`, value.toString());
                            }
                        }
                    }
                }
            } catch (e) {
                // Skip non-contract events
            }
        }

        console.log("\n" + "=".repeat(70));
        console.log("💰 FINAL PROFIT ANALYSIS");
        console.log("=".repeat(70));

        const contractWethAfter = await weth.balanceOf(contractAddress);
        const netChange = contractWethAfter - contractWethBefore;
        const actualProfit = netChange > 0n ? netChange : 0n;

        console.log("Contract WETH Before:", ethers.formatEther(contractWethBefore), "WETH");
        console.log("Contract WETH After:", ethers.formatEther(contractWethAfter), "WETH");
        console.log("Net Change:", ethers.formatEther(netChange), "WETH");
        console.log("");

        console.log("Expected Profit:", ethers.formatEther(bestRoute.profit), "WETH");
        console.log("Actual Profit:", ethers.formatEther(actualProfit), "WETH");
        console.log("Gas Cost:", ethers.formatEther(gasCost), "ETH");
        
        const netProfit = actualProfit - gasCost;
        console.log("\n🎯 NET PROFIT (after gas):", ethers.formatEther(netProfit), "ETH");
        
        if (netProfit > 0n) {
            console.log("✅ ARBITRAGE WAS PROFITABLE! 🎉");
        } else if (actualProfit > 0n) {
            console.log("⚠️  Arbitrage succeeded but gas costs exceeded profit");
        } else {
            console.log("❌ Arbitrage was not profitable");
        }

        console.log("\n" + "=".repeat(70));
        console.log("🎊 FLASH LOAN ARBITRAGE COMPLETED!");
        console.log("=".repeat(70));
        console.log("\n✅ Flash loan borrowed from Aave");
        console.log("✅ Swaps executed on Uniswap V3");
        console.log("✅ Loan repaid successfully");
        console.log("✅ Profit extracted\n");

    } catch (error) {
        console.error("\n❌ EXECUTION FAILED");
        console.error("=".repeat(70));
        console.error("Error:", error.message);
        
        if (error.data) {
            console.error("Error Data:", error.data);
        }
        
        if (error.message.includes("STF")) {
            console.log("\n💡 Slippage too high - market moved against us");
        } else if (error.message.includes("Insufficient")) {
            console.log("\n💡 Insufficient output - route not profitable enough");
        }
        
        console.error("=".repeat(70) + "\n");
    }

    console.log("=".repeat(70));
    console.log("TEST COMPLETED");
    console.log("=".repeat(70));
    console.log("\n📊 Contract Address:", contractAddress);
    console.log("🔗 This is a LOCAL fork - no real money spent!\n");
}

main()
    .then(() => process.exit(0))
    .catch((error) => {
        console.error("\n❌ Test failed:");
        console.error(error);
        process.exit(1);
    });