const { ethers } = require("hardhat");

/**
 * REAL-TIME QUOTE CHECKER
 * 
 * Quickly check quotes for specific routes before executing
 * Validates profitability with current market conditions
 * 
 * Run: npx hardhat run scripts/check-quotes.js --network localhost
 */

const MAINNET_CONFIG = {
    UNISWAP_V3_QUOTER_V2: "0xb27308f9F90D607463bb33eA1BeBb41C27CE5AB6",
    WETH: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
    USDC: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
    USDT: "0xdAC17F958D2ee523a2206206994597C13D831ec7",
    DAI: "0x6B175474E89094C44Da98b954EedeAC495271d0F",
};

const AAVE_FEE_BPS = 9; // 0.09%

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
            sqrtPriceX96After: result[1],
            initializedTicksCrossed: result[2],
            gasEstimate: result[3]
        };
    } catch (error) {
        // Try to extract revert reason
        let reason = "Unknown error";
        if (error.message.includes("insufficient liquidity")) {
            reason = "Insufficient liquidity in pool";
        } else if (error.message.includes("Too little received")) {
            reason = "Price impact too high";
        } else if (error.message) {
            reason = error.message;
        }
        
        return {
            success: false,
            amountOut: 0n,
            error: reason
        };
    }
}

function formatToken(amount, decimals, symbol) {
    return `${ethers.formatUnits(amount, decimals)} ${symbol}`;
}

async function checkRoute(quoter, config) {
    const { name, tokenIn, tokenOut, intermediate, fee1, fee2, amount } = config;
    
    console.log("\n" + "=".repeat(80));
    console.log(`📊 Checking: ${name}`);
    console.log("=".repeat(80));
    
    // Step 1: First swap
    console.log("\n🔄 Step 1: Getting quote for first swap...");
    const quote1 = await getQuote(quoter, tokenIn, intermediate, fee1, amount);
    
    if (!quote1.success) {
        console.log(`❌ First swap failed: ${quote1.error}`);
        return null;
    }
    
    console.log(`✅ First swap quote received`);
    console.log(`   Input:  ${formatToken(amount, 18, "WETH")}`);
    
    let intermediateSymbol, intermediateDecimals;
    if (intermediate === MAINNET_CONFIG.USDC || intermediate === MAINNET_CONFIG.USDT) {
        intermediateSymbol = intermediate === MAINNET_CONFIG.USDC ? "USDC" : "USDT";
        intermediateDecimals = 6;
    } else {
        intermediateSymbol = "DAI";
        intermediateDecimals = 18;
    }
    
    console.log(`   Output: ${formatToken(quote1.amountOut, intermediateDecimals, intermediateSymbol)}`);
    console.log(`   Fee:    ${fee1 / 10000}%`);
    console.log(`   Gas:    ${quote1.gasEstimate.toString()}`);
    
    // Step 2: Second swap
    console.log("\n🔄 Step 2: Getting quote for second swap...");
    const quote2 = await getQuote(quoter, intermediate, tokenOut, fee2, quote1.amountOut);
    
    if (!quote2.success) {
        console.log(`❌ Second swap failed: ${quote2.error}`);
        return null;
    }
    
    console.log(`✅ Second swap quote received`);
    console.log(`   Input:  ${formatToken(quote1.amountOut, intermediateDecimals, intermediateSymbol)}`);
    console.log(`   Output: ${formatToken(quote2.amountOut, 18, "WETH")}`);
    console.log(`   Fee:    ${fee2 / 10000}%`);
    console.log(`   Gas:    ${quote2.gasEstimate.toString()}`);
    
    // Calculate profitability
    console.log("\n" + "─".repeat(80));
    console.log("💰 PROFITABILITY ANALYSIS");
    console.log("─".repeat(80));
    
    const aaveFee = (amount * BigInt(AAVE_FEE_BPS)) / 10000n;
    const totalDebt = amount + aaveFee;
    const amountReturned = quote2.amountOut;
    const grossProfit = amountReturned > totalDebt ? amountReturned - totalDebt : 0n;
    const profitPercent = Number(grossProfit) / Number(amount) * 100;
    
    console.log(`Flash Loan Amount:    ${formatToken(amount, 18, "WETH")}`);
    console.log(`Aave Fee (0.09%):     ${formatToken(aaveFee, 18, "WETH")}`);
    console.log(`Total Debt:           ${formatToken(totalDebt, 18, "WETH")}`);
    console.log(`Amount Returned:      ${formatToken(amountReturned, 18, "WETH")}`);
    console.log(`Gross Profit:         ${formatToken(grossProfit, 18, "WETH")}`);
    console.log(`Profit Percentage:    ${profitPercent.toFixed(6)}%`);
    
    // Estimate gas costs
    const totalGasEstimate = quote1.gasEstimate + quote2.gasEstimate;
    const flashLoanOverhead = 200000n; // Additional gas for flash loan
    const totalGas = totalGasEstimate + flashLoanOverhead;
    
    console.log("\n" + "─".repeat(80));
    console.log("⛽ GAS COST ESTIMATION");
    console.log("─".repeat(80));
    
    const gasPrices = [10n, 20n, 30n, 50n, 100n]; // Different gas prices in gwei
    
    console.log(`Estimated Gas Units:  ${totalGas.toString()}`);
    console.log(`\nNet Profit at different gas prices:`);
    
    for (const gasPriceGwei of gasPrices) {
        const gasPrice = ethers.parseUnits(gasPriceGwei.toString(), "gwei");
        const gasCost = totalGas * gasPrice;
        const netProfit = grossProfit - gasCost;
        const netProfitPercent = Number(netProfit) / Number(amount) * 100;
        
        const status = netProfit > 0n ? "✅" : "❌";
        console.log(`  ${gasPriceGwei.toString().padStart(3)} gwei: ${status} ${formatToken(netProfit, 18, "WETH")} (${netProfitPercent.toFixed(6)}%)`);
    }
    
    // Calculate recommended minimum amounts with slippage
    console.log("\n" + "─".repeat(80));
    console.log("🎯 RECOMMENDED EXECUTION PARAMETERS");
    console.log("─".repeat(80));
    
    const slippages = [50, 100, 200, 500]; // 0.5%, 1%, 2%, 5%
    
    console.log("\nMinimum output amounts for different slippage tolerances:");
    for (const slippageBps of slippages) {
        const minAmount1 = (quote1.amountOut * (10000n - BigInt(slippageBps))) / 10000n;
        const minAmount2 = (quote2.amountOut * (10000n - BigInt(slippageBps))) / 10000n;
        
        const wouldCoverDebt = minAmount2 >= totalDebt;
        const status = wouldCoverDebt ? "✅" : "❌";
        
        console.log(`\n  Slippage: ${slippageBps / 100}%`);
        console.log(`    Swap 1 Min: ${formatToken(minAmount1, intermediateDecimals, intermediateSymbol)} ${status}`);
        console.log(`    Swap 2 Min: ${formatToken(minAmount2, 18, "WETH")} ${wouldCoverDebt ? "(Covers debt)" : "(⚠️  Does NOT cover debt!)"}`);
    }
    
    // Final recommendation
    console.log("\n" + "=".repeat(80));
    console.log("📋 EXECUTION RECOMMENDATION");
    console.log("=".repeat(80));
    
    const isProfitable = grossProfit > 0n;
    const minGasForProfit = grossProfit > 0n ? (grossProfit * 10n ** 9n) / totalGas : 0n;
    
    if (!isProfitable) {
        console.log("\n❌ NOT RECOMMENDED - Route is not profitable");
        console.log(`   Loss: ${formatToken(-grossProfit, 18, "WETH")}`);
    } else {
        console.log("\n✅ Route shows potential profit");
        console.log(`   Gross Profit: ${formatToken(grossProfit, 18, "WETH")}`);
        console.log(`   Max Gas Price: ${ethers.formatUnits(minGasForProfit, "gwei")} gwei (to remain profitable)`);
        
        // Check if profitable at current typical gas prices
        const currentGasPrice = 30n; // Assume 30 gwei as typical
        const gasCostAt30 = totalGas * ethers.parseUnits(currentGasPrice.toString(), "gwei");
        const netAt30 = grossProfit - gasCostAt30;
        
        if (netAt30 > 0n) {
            console.log(`   ✅ Profitable at typical gas (30 gwei): ${formatToken(netAt30, 18, "WETH")}`);
            console.log("\n   💡 EXECUTE THIS ROUTE!");
        } else {
            console.log(`   ⚠️  Not profitable at 30 gwei gas price`);
            console.log(`   Wait for gas prices below ${ethers.formatUnits(minGasForProfit, "gwei")} gwei`);
        }
    }
    
    return {
        isProfitable,
        grossProfit,
        profitPercent,
        quote1: quote1.amountOut,
        quote2: quote2.amountOut,
        totalDebt,
        estimatedGas: totalGas
    };
}

async function main() {
    console.log("=".repeat(80));
    console.log("REAL-TIME QUOTE CHECKER");
    console.log("=".repeat(80));
    console.log("Network:", hre.network.name);
    console.log("=".repeat(80));
    
    const quoterABI = [
        "function quoteExactInputSingle((address tokenIn, address tokenOut, uint256 amountIn, uint24 fee, uint160 sqrtPriceLimitX96)) external returns (uint256 amountOut, uint160 sqrtPriceX96After, uint32 initializedTicksCrossed, uint256 gasEstimate)"
    ];
    
    const quoter = await ethers.getContractAt(
        quoterABI,
        MAINNET_CONFIG.UNISWAP_V3_QUOTER_V2
    );
    
    // Define routes to check
    const routesToCheck = [
        {
            name: "WETH → USDC → WETH (0.3% → 0.05%)",
            tokenIn: MAINNET_CONFIG.WETH,
            intermediate: MAINNET_CONFIG.USDC,
            tokenOut: MAINNET_CONFIG.WETH,
            fee1: 3000,
            fee2: 500,
            amount: ethers.parseEther("1") // 1 ETH
        },
        {
            name: "WETH → USDC → WETH (0.05% → 0.3%)",
            tokenIn: MAINNET_CONFIG.WETH,
            intermediate: MAINNET_CONFIG.USDC,
            tokenOut: MAINNET_CONFIG.WETH,
            fee1: 500,
            fee2: 3000,
            amount: ethers.parseEther("1")
        },
        {
            name: "WETH → USDT → WETH (0.3% → 0.05%)",
            tokenIn: MAINNET_CONFIG.WETH,
            intermediate: MAINNET_CONFIG.USDT,
            tokenOut: MAINNET_CONFIG.WETH,
            fee1: 3000,
            fee2: 500,
            amount: ethers.parseEther("1")
        },
        {
            name: "WETH → DAI → WETH (0.3% → 0.05%)",
            tokenIn: MAINNET_CONFIG.WETH,
            intermediate: MAINNET_CONFIG.DAI,
            tokenOut: MAINNET_CONFIG.WETH,
            fee1: 3000,
            fee2: 500,
            amount: ethers.parseEther("1")
        },
    ];
    
    console.log(`\nChecking ${routesToCheck.length} routes...\n`);
    
    const results = [];
    
    for (const route of routesToCheck) {
        const result = await checkRoute(quoter, route);
        if (result) {
            results.push({ ...route, ...result });
        }
        
        // Small delay between checks
        await new Promise(resolve => setTimeout(resolve, 500));
    }
    
    // Summary
    console.log("\n\n" + "=".repeat(80));
    console.log("📊 SUMMARY");
    console.log("=".repeat(80));
    
    const profitable = results.filter(r => r.isProfitable);
    
    console.log(`\nRoutes Checked: ${results.length}`);
    console.log(`Profitable Routes: ${profitable.length}`);
    console.log(`Unprofitable Routes: ${results.length - profitable.length}`);
    
    if (profitable.length > 0) {
        console.log("\n🏆 Best Opportunities:");
        profitable
            .sort((a, b) => Number(b.grossProfit - a.grossProfit))
            .slice(0, 3)
            .forEach((route, idx) => {
                console.log(`\n${idx + 1}. ${route.name}`);
                console.log(`   Profit: ${formatToken(route.grossProfit, 18, "WETH")} (${route.profitPercent.toFixed(4)}%)`);
            });
    } else {
        console.log("\n❌ No profitable routes found at current market prices");
    }
    
    console.log("\n" + "=".repeat(80));
    console.log("CHECK COMPLETE");
    console.log("=".repeat(80) + "\n");
}

main()
    .then(() => process.exit(0))
    .catch((error) => {
        console.error("\n❌ Check failed:");
        console.error(error);
        process.exit(1);
    });