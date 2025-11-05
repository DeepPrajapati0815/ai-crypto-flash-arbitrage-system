const { ethers } = require("hardhat");

/**
 * ADVANCED ROUTE SCANNER
 * 
 * Scans multiple DEXes and routes to find profitable arbitrage opportunities
 * Tests various amounts and token pairs
 * 
 * Run: npx hardhat run scripts/scan-profitable-routes.js --network localhost
 */

const MAINNET_CONFIG = {
    UNISWAP_V3_QUOTER_V2: "0xb27308f9F90D607463bb33eA1BeBb41C27CE5AB6",
    
    WETH: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
    USDC: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
    USDT: "0xdAC17F958D2ee523a2206206994597C13D831ec7",
    DAI: "0x6B175474E89094C44Da98b954EedeAC495271d0F",
    WBTC: "0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599",
};

const POOL_FEES = {
    LOWEST: 100,    // 0.01%
    LOW: 500,       // 0.05%
    MEDIUM: 3000,   // 0.3%
    HIGH: 10000     // 1%
};

const TOKEN_INFO = {
    [MAINNET_CONFIG.WETH]: { symbol: "WETH", decimals: 18 },
    [MAINNET_CONFIG.USDC]: { symbol: "USDC", decimals: 6 },
    [MAINNET_CONFIG.USDT]: { symbol: "USDT", decimals: 6 },
    [MAINNET_CONFIG.DAI]: { symbol: "DAI", decimals: 18 },
    [MAINNET_CONFIG.WBTC]: { symbol: "WBTC", decimals: 8 },
};

function formatTokenAmount(amount, tokenAddress) {
    const info = TOKEN_INFO[tokenAddress];
    if (!info) return amount.toString();
    return ethers.formatUnits(amount, info.decimals) + " " + info.symbol;
}

async function getQuote(quoter, tokenIn, tokenOut, fee, amountIn) {
    try {
        const params = {
            tokenIn,
            tokenOut,
            amountIn,
            fee,
            sqrtPriceLimitX96: 0
        };
        
        const result = await quoter.quoteExactInputSingle.staticCall(params);
        return {
            success: true,
            amountOut: result[0],
            gasEstimate: result[3]
        };
    } catch (error) {
        return {
            success: false,
            amountOut: 0n,
            gasEstimate: 0n,
            error: error.message
        };
    }
}

async function testArbitrageRoute(quoter, config) {
    const { tokenIn, intermediate, tokenOut, fee1, fee2, amount, name } = config;
    
    // Get quote for first swap
    const quote1 = await getQuote(quoter, tokenIn, intermediate, fee1, amount);
    if (!quote1.success) {
        return null;
    }
    
    // Get quote for second swap
    const quote2 = await getQuote(quoter, intermediate, tokenOut, fee2, quote1.amountOut);
    if (!quote2.success) {
        return null;
    }
    
    // Calculate profitability (only for WETH → X → WETH routes)
    if (tokenIn === MAINNET_CONFIG.WETH && tokenOut === MAINNET_CONFIG.WETH) {
        const aaveFee = amount * 9n / 10000n; // 0.09%
        const totalDebt = amount + aaveFee;
        const profit = quote2.amountOut > totalDebt ? quote2.amountOut - totalDebt : 0n;
        const profitPercent = Number(profit) / Number(amount) * 100;
        
        return {
            name,
            tokenIn,
            intermediate,
            tokenOut,
            fee1,
            fee2,
            amount,
            amountIntermediate: quote1.amountOut,
            amountOut: quote2.amountOut,
            totalDebt,
            profit,
            profitPercent,
            isProfitable: profit > 0n,
            estimatedGas: quote1.gasEstimate + quote2.gasEstimate
        };
    }
    
    return null;
}

async function scanAllRoutes(quoter, testAmounts) {
    console.log("\n" + "=".repeat(80));
    console.log("🔍 COMPREHENSIVE ROUTE SCANNER");
    console.log("=".repeat(80));
    
    const intermediateTokens = [
        MAINNET_CONFIG.USDC,
        MAINNET_CONFIG.USDT,
        MAINNET_CONFIG.DAI,
    ];
    
    const fees = [POOL_FEES.LOW, POOL_FEES.MEDIUM, POOL_FEES.HIGH];
    
    const allResults = [];
    let routesTested = 0;
    let routesSucceeded = 0;
    
    for (const amount of testAmounts) {
        console.log(`\n${"─".repeat(80)}`);
        console.log(`Testing with ${ethers.formatEther(amount)} WETH flash loan`);
        console.log("─".repeat(80));
        
        for (const intermediate of intermediateTokens) {
            const intermediateSymbol = TOKEN_INFO[intermediate].symbol;
            
            for (const fee1 of fees) {
                for (const fee2 of fees) {
                    routesTested++;
                    
                    const config = {
                        name: `WETH → ${intermediateSymbol} → WETH (${fee1/100}bp → ${fee2/100}bp)`,
                        tokenIn: MAINNET_CONFIG.WETH,
                        intermediate,
                        tokenOut: MAINNET_CONFIG.WETH,
                        fee1,
                        fee2,
                        amount
                    };
                    
                    const result = await testArbitrageRoute(quoter, config);
                    
                    if (result) {
                        routesSucceeded++;
                        allResults.push(result);
                        
                        if (result.isProfitable) {
                            console.log(`\n✅ ${result.name}`);
                            console.log(`   Profit: ${ethers.formatEther(result.profit)} WETH (${result.profitPercent.toFixed(4)}%)`);
                        }
                    }
                    
                    // Small delay to avoid rate limiting
                    await new Promise(resolve => setTimeout(resolve, 50));
                }
            }
        }
    }
    
    console.log("\n" + "=".repeat(80));
    console.log("📊 SCAN SUMMARY");
    console.log("=".repeat(80));
    console.log(`Routes Tested: ${routesTested}`);
    console.log(`Routes Succeeded: ${routesSucceeded}`);
    console.log(`Routes Failed: ${routesTested - routesSucceeded}`);
    
    const profitableRoutes = allResults.filter(r => r.isProfitable);
    console.log(`Profitable Routes: ${profitableRoutes.length}`);
    
    return allResults;
}

async function displayTopRoutes(results, count = 10) {
    const profitable = results.filter(r => r.isProfitable);
    
    if (profitable.length === 0) {
        console.log("\n❌ NO PROFITABLE ROUTES FOUND");
        console.log("\nPossible reasons:");
        console.log("  • Market is efficient - arbitrage opportunities are rare on mainnet");
        console.log("  • Pool fees exceed potential profit");
        console.log("  • Need larger flash loan amounts");
        console.log("  • Need cross-DEX opportunities (Uniswap vs Sushiswap)");
        console.log("\nSuggestions:");
        console.log("  • Try testing during high volatility");
        console.log("  • Use larger amounts (1-10 ETH)");
        console.log("  • Monitor mempool for opportunities");
        console.log("  • Check triangular arbitrage (3+ hops)");
        return;
    }
    
    // Sort by profit
    profitable.sort((a, b) => Number(b.profit - a.profit));
    
    console.log("\n" + "=".repeat(80));
    console.log(`🏆 TOP ${Math.min(count, profitable.length)} PROFITABLE ROUTES`);
    console.log("=".repeat(80));
    
    profitable.slice(0, count).forEach((route, idx) => {
        console.log(`\n${idx + 1}. ${route.name}`);
        console.log(`   Amount: ${ethers.formatEther(route.amount)} WETH`);
        console.log(`   Route Output: ${ethers.formatEther(route.amountOut)} WETH`);
        console.log(`   Debt to Repay: ${ethers.formatEther(route.totalDebt)} WETH`);
        console.log(`   Gross Profit: ${ethers.formatEther(route.profit)} WETH`);
        console.log(`   Profit %: ${route.profitPercent.toFixed(4)}%`);
        console.log(`   Est. Gas: ${route.estimatedGas.toString()}`);
        
        // Estimate gas cost impact
        const gasPrice = ethers.parseUnits("30", "gwei"); // Assume 30 gwei
        const estimatedGasCost = route.estimatedGas * gasPrice;
        const netProfit = route.profit - estimatedGasCost;
        
        console.log(`   Est. Gas Cost: ${ethers.formatEther(estimatedGasCost)} ETH @ 30 gwei`);
        console.log(`   Net Profit: ${ethers.formatEther(netProfit)} ETH`);
        
        if (netProfit > 0n) {
            console.log(`   ✅ PROFITABLE after gas`);
        } else {
            console.log(`   ⚠️  Gas costs exceed profit`);
        }
    });
    
    // Show statistics
    console.log("\n" + "=".repeat(80));
    console.log("📈 PROFIT STATISTICS");
    console.log("=".repeat(80));
    
    const totalProfit = profitable.reduce((sum, r) => sum + r.profit, 0n);
    const avgProfit = totalProfit / BigInt(profitable.length);
    const maxProfit = profitable[0].profit;
    const minProfit = profitable[profitable.length - 1].profit;
    
    console.log(`Total Opportunities: ${profitable.length}`);
    console.log(`Total Potential Profit: ${ethers.formatEther(totalProfit)} ETH`);
    console.log(`Average Profit: ${ethers.formatEther(avgProfit)} ETH`);
    console.log(`Max Profit: ${ethers.formatEther(maxProfit)} ETH`);
    console.log(`Min Profit: ${ethers.formatEther(minProfit)} ETH`);
    
    // Group by intermediate token
    const byToken = {};
    profitable.forEach(r => {
        const symbol = TOKEN_INFO[r.intermediate].symbol;
        if (!byToken[symbol]) byToken[symbol] = [];
        byToken[symbol].push(r);
    });
    
    console.log("\n" + "─".repeat(80));
    console.log("Profitable Routes by Intermediate Token:");
    for (const [token, routes] of Object.entries(byToken)) {
        const totalProfit = routes.reduce((sum, r) => sum + r.profit, 0n);
        console.log(`  ${token}: ${routes.length} routes, ${ethers.formatEther(totalProfit)} ETH total profit`);
    }
}

async function main() {
    console.log("=".repeat(80));
    console.log("ADVANCED ARBITRAGE ROUTE SCANNER");
    console.log("=".repeat(80));
    console.log("Network:", hre.network.name);
    console.log("=".repeat(80));
    
    // Initialize quoter
    const quoterABI = [
        "function quoteExactInputSingle((address tokenIn, address tokenOut, uint256 amountIn, uint24 fee, uint160 sqrtPriceLimitX96)) external returns (uint256 amountOut, uint160 sqrtPriceX96After, uint32 initializedTicksCrossed, uint256 gasEstimate)"
    ];
    
    const quoter = await ethers.getContractAt(
        quoterABI,
        MAINNET_CONFIG.UNISWAP_V3_QUOTER_V2
    );
    
    // Test with multiple amounts
    const testAmounts = [
        ethers.parseEther("0.1"),   // 0.1 ETH
        ethers.parseEther("0.5"),   // 0.5 ETH
        ethers.parseEther("1"),     // 1 ETH
        ethers.parseEther("5"),     // 5 ETH
        ethers.parseEther("10"),    // 10 ETH
    ];
    
    console.log("\nTest Amounts:");
    testAmounts.forEach(amt => {
        console.log(`  • ${ethers.formatEther(amt)} ETH`);
    });
    
    console.log("\nIntermediate Tokens:");
    console.log("  • USDC");
    console.log("  • USDT");
    console.log("  • DAI");
    
    console.log("\nPool Fees:");
    console.log("  • 0.05% (5 bp)");
    console.log("  • 0.3% (30 bp)");
    console.log("  • 1% (100 bp)");
    
    console.log(`\nTotal Combinations: ${testAmounts.length} × 3 tokens × 9 fee pairs = ${testAmounts.length * 3 * 9} routes`);
    
    // Scan all routes
    const startTime = Date.now();
    const results = await scanAllRoutes(quoter, testAmounts);
    const endTime = Date.now();
    
    console.log(`\n⏱️  Scan completed in ${((endTime - startTime) / 1000).toFixed(2)} seconds`);
    
    // Display top routes
    await displayTopRoutes(results, 15);
    
    // Export results to JSON
    const profitable = results.filter(r => r.isProfitable);
    if (profitable.length > 0) {
        console.log("\n" + "=".repeat(80));
        console.log("💾 EXPORT BEST ROUTE");
        console.log("=".repeat(80));
        
        const bestRoute = profitable[0];
        const exportData = {
            route: bestRoute.name,
            tokenIn: bestRoute.tokenIn,
            intermediate: bestRoute.intermediate,
            tokenOut: bestRoute.tokenOut,
            fee1: bestRoute.fee1,
            fee2: bestRoute.fee2,
            amount: bestRoute.amount.toString(),
            expectedProfit: bestRoute.profit.toString(),
            profitPercent: bestRoute.profitPercent,
            minAmountIntermediate: (bestRoute.amountIntermediate * 995n / 1000n).toString(), // 0.5% slippage
            minAmountOut: (bestRoute.amountOut * 995n / 1000n).toString() // 0.5% slippage
        };
        
        console.log("\nBest Route Configuration:");
        console.log(JSON.stringify(exportData, null, 2));
        console.log("\n💡 Copy this configuration to use in your arbitrage execution script");
    }
    
    console.log("\n" + "=".repeat(80));
    console.log("SCAN COMPLETE");
    console.log("=".repeat(80) + "\n");
}

main()
    .then(() => process.exit(0))
    .catch((error) => {
        console.error("\n❌ Scanner failed:");
        console.error(error);
        process.exit(1);
    });