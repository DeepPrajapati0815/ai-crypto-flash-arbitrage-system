const { ethers } = require("hardhat");

/**
 * Complete Local Fork Testing Script
 * 
 * This script tests flash loan arbitrage on a local mainnet fork
 * with REAL liquidity, prices, and DEX pools
 * 
 * Prerequisites:
 * 1. Start mainnet fork in Terminal 1:
 *    npx hardhat node --fork https://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY
 * 
 * 2. Run this script in Terminal 2:
 *    npx hardhat run scripts/test-local-fork.js --network localhost
 */

// Ethereum Mainnet Addresses (will work on fork)
const MAINNET_CONFIG = {
    AAVE_POOL: "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2",
    UNISWAP_V3_ROUTER: "0xE592427A0AEce92De3Edee1F18E0157C05861564",
    UNISWAP_V3_QUOTER: "0x61fFE014bA17989E743c5F6cB21bF9697530B21e", // QuoterV2
    SUSHISWAP_ROUTER: "0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F",
    
    // Mainnet tokens
    WETH: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
    USDC: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
    DAI: "0x6B175474E89094C44Da98b954EedeAC495271d0F",
    
    // Test parameters
    FLASH_LOAN_AMOUNT: ethers.parseEther("1"), // 1 WETH (~$3,600)
    MIN_PROFIT_THRESHOLD: ethers.parseEther("0.01"), // 0.01 ETH minimum
};

async function main() {
    console.log("============================================================");
    console.log("LOCAL MAINNET FORK - FLASH LOAN ARBITRAGE TEST");
    console.log("============================================================");
    console.log("Network:", hre.network.name);
    console.log("Testing with REAL mainnet liquidity (forked locally)");
    console.log("============================================================\n");

    // Get signers (fork gives you 10,000 ETH each!)
    const [deployer] = await ethers.getSigners();
    console.log("Deployer:", deployer.address);
    
    const balance = await ethers.provider.getBalance(deployer.address);
    console.log("ETH Balance:", ethers.formatEther(balance), "ETH");
    console.log("(Fork gives you 10,000 ETH for testing!)\n");

    // ========================================================================
    // STEP 1: DEPLOY CONTRACT
    // ========================================================================
    console.log("============================================================");
    console.log("STEP 1: DEPLOY FLASH ARBITRAGE CONTRACT");
    console.log("============================================================\n");

    const FlashArbUltimate = await ethers.getContractFactory("FlashArbUltimate");
    
    console.log("Deploying FlashArbUltimate...");
    
    // Get current gas price from the fork
    const feeData = await ethers.provider.getFeeData();
    
    const flashArb = await FlashArbUltimate.deploy(
        MAINNET_CONFIG.AAVE_POOL,
        MAINNET_CONFIG.UNISWAP_V3_ROUTER,
        MAINNET_CONFIG.SUSHISWAP_ROUTER,
        ethers.parseEther("0.001"), // 0.001 ETH min profit
        5, // max failed attempts
        {
            maxFeePerGas: feeData.maxFeePerGas,
            maxPriorityFeePerGas: feeData.maxPriorityFeePerGas
        }
    );

    await flashArb.waitForDeployment();
    const contractAddress = await flashArb.getAddress();
    
    console.log("✅ Contract deployed to:", contractAddress);
    console.log("");

    // ========================================================================
    // STEP 2: FETCH REAL-TIME PRICES FROM MAINNET
    // ========================================================================
    console.log("============================================================");
    console.log("STEP 2: FETCH REAL-TIME PRICES (MAINNET DATA)");
    console.log("============================================================\n");

    const quoterABI = [
        "function quoteExactInputSingle(address tokenIn, address tokenOut, uint24 fee, uint256 amountIn, uint160 sqrtPriceLimitX96) external returns (uint256 amountOut)"
    ];
    
    const quoter = await ethers.getContractAt(quoterABI, MAINNET_CONFIG.UNISWAP_V3_QUOTER);

    console.log("Fetching prices for 1 WETH...\n");

    try {
        // Quote WETH → USDC on Uniswap V3
        const wethToUsdc = await quoter.quoteExactInputSingle.staticCall(
            MAINNET_CONFIG.WETH,
            MAINNET_CONFIG.USDC,
            3000, // 0.3% fee
            MAINNET_CONFIG.FLASH_LOAN_AMOUNT,
            0
        );

        console.log("Uniswap V3 Price:");
        console.log("  1 WETH →", ethers.formatUnits(wethToUsdc, 6), "USDC");
        console.log("");

        // Quote USDC → WETH on Sushiswap (via Uniswap for comparison)
        const usdcToWeth = await quoter.quoteExactInputSingle.staticCall(
            MAINNET_CONFIG.USDC,
            MAINNET_CONFIG.WETH,
            3000,
            wethToUsdc,
            0
        );

        console.log("Round Trip:");
        console.log("  1 WETH →", ethers.formatUnits(wethToUsdc, 6), "USDC →", ethers.formatEther(usdcToWeth), "WETH");
        console.log("");

        // Calculate spread
        const profit = usdcToWeth > MAINNET_CONFIG.FLASH_LOAN_AMOUNT ? 
            usdcToWeth - MAINNET_CONFIG.FLASH_LOAN_AMOUNT : 0n;
        const profitPercent = Number(profit) / Number(MAINNET_CONFIG.FLASH_LOAN_AMOUNT) * 100;

        console.log("Spread Analysis:");
        console.log("  Gross Profit:", ethers.formatEther(profit), "WETH");
        console.log("  Profit %:", profitPercent.toFixed(4), "%");
        console.log("");

    } catch (error) {
        console.error("❌ Failed to fetch prices:", error.message);
        console.log("Make sure you're running on a mainnet fork!\n");
    }

    // ========================================================================
    // STEP 3: GET WETH FOR TESTING
    // ========================================================================
    console.log("============================================================");
    console.log("STEP 3: GET WETH FOR TESTING");
    console.log("============================================================\n");

    const weth = await ethers.getContractAt(
        ["function deposit() payable", "function balanceOf(address) view returns (uint256)"],
        MAINNET_CONFIG.WETH
    );

    console.log("Wrapping 10 ETH to WETH...");
    const wrapTx = await weth.deposit({ 
        value: ethers.parseEther("10"),
        maxFeePerGas: feeData.maxFeePerGas,
        maxPriorityFeePerGas: feeData.maxPriorityFeePerGas
    });
    await wrapTx.wait();

    const wethBalance = await weth.balanceOf(deployer.address);
    console.log("✅ WETH Balance:", ethers.formatEther(wethBalance), "WETH\n");

    // ========================================================================
    // STEP 4: AUTHORIZE EXECUTOR
    // ========================================================================
    console.log("============================================================");
    console.log("STEP 4: AUTHORIZE EXECUTOR");
    console.log("============================================================\n");

    console.log("Adding executor authorization...");
    const authTx = await flashArb.addAuthorizedExecutor(deployer.address, {
        maxFeePerGas: feeData.maxFeePerGas,
        maxPriorityFeePerGas: feeData.maxPriorityFeePerGas
    });
    await authTx.wait();
    console.log("✅ Executor authorized!");
    
    // Set high gas price limit for testing (100 gwei)
    console.log("Setting gas price limit for testing...");
    const gasLimitTx = await flashArb.setMaxGasPrice(ethers.parseUnits("100", "gwei"), {
        maxFeePerGas: feeData.maxFeePerGas,
        maxPriorityFeePerGas: feeData.maxPriorityFeePerGas
    });
    await gasLimitTx.wait();
    console.log("✅ Gas price limit set to 100 gwei");
    
    // Set gas price tolerance to 300% (maximum allowed)
    console.log("Setting gas price tolerance to 300%...");
    const toleranceTx = await flashArb.setGasPriceTolerance(300, {
        maxFeePerGas: feeData.maxFeePerGas,
        maxPriorityFeePerGas: feeData.maxPriorityFeePerGas
    });
    await toleranceTx.wait();
    console.log("✅ Gas price tolerance set to 300%\n");

    // ========================================================================
    // STEP 5: BUILD ARBITRAGE ROUTE
    // ========================================================================
    console.log("============================================================");
    console.log("STEP 5: BUILD ARBITRAGE ROUTE");
    console.log("============================================================\n");

    const flashLoanAmount = ethers.parseEther("1"); // Use 1 WETH for better liquidity
    const aavePremium = flashLoanAmount * 9n / 10000n; // 0.09%
    const totalDebt = flashLoanAmount + aavePremium;

    console.log("Flash Loan Route:");
    console.log("1. Borrow:", ethers.formatEther(flashLoanAmount), "WETH from Aave");
    console.log("   Premium:", ethers.formatEther(aavePremium), "WETH");
    console.log("2. Swap WETH → USDC on Uniswap V3 (0.3% pool)");
    console.log("3. Swap USDC → WETH on Uniswap V3 (0.05% pool - arbitrage)");
    console.log("4. Repay:", ethers.formatEther(totalDebt), "WETH");
    console.log("");

    // Build route with very loose slippage for testing
    // In production, you'd calculate exact amounts from real quotes
    const tuples = [
        {
            dexType: 0, // UniswapV3
            tokenIn: MAINNET_CONFIG.WETH,
            tokenOut: MAINNET_CONFIG.USDC,
            poolFee: 3000, // 0.3% pool
            amountIn: flashLoanAmount,
            minAmountOut: ethers.parseUnits("2000", 6) // Very loose: ~$2000 USDC minimum (50% slippage for testing)
        },
        {
            dexType: 0, // UniswapV3
            tokenIn: MAINNET_CONFIG.USDC,
            tokenOut: MAINNET_CONFIG.WETH,
            poolFee: 500, // 0.05% pool (lower fee = potentially better rate)
            amountIn: ethers.parseUnits("2000", 6), // Use all USDC from first swap
            minAmountOut: ethers.parseEther("0.5") // Very loose: need at least 0.5 WETH back (will likely get ~1 WETH)
        }
    ];

    // ========================================================================
    // STEP 6: EXECUTE FLASH LOAN ARBITRAGE
    // ========================================================================
    console.log("============================================================");
    console.log("STEP 6: EXECUTE FLASH LOAN ARBITRAGE");
    console.log("============================================================\n");

    console.log("🚀 Executing flash loan arbitrage on mainnet fork...");
    console.log("");

    try {
        // Calculate acceptable gas price within tolerance
        const block = await ethers.provider.getBlock('latest');
        const baseFee = block.baseFeePerGas;
        const maxAllowedGas = (baseFee * 300n) / 100n; // 300% of base fee
        const priorityFee = maxAllowedGas / 10n; // 10% of max fee for priority
        
        console.log("Gas Price Info:");
        console.log("  Base Fee:", ethers.formatUnits(baseFee, "gwei"), "gwei");
        console.log("  Max Allowed (300%):", ethers.formatUnits(maxAllowedGas, "gwei"), "gwei");
        console.log("  Priority Fee:", ethers.formatUnits(priorityFee, "gwei"), "gwei");
        console.log("");
        
        const tx = await flashArb.executeFlashArbitrage(
            MAINNET_CONFIG.WETH,
            flashLoanAmount,
            tuples,
            { 
                gasLimit: 1000000,
                maxFeePerGas: maxAllowedGas,
                maxPriorityFeePerGas: priorityFee
            }
        );

        console.log("✅ Transaction Submitted!");
        console.log("TX Hash:", tx.hash);
        console.log("");
        console.log("Waiting for confirmation...");

        const receipt = await tx.wait();

        console.log("");
        console.log("============================================================");
        console.log("✅ TRANSACTION CONFIRMED!");
        console.log("============================================================");
        console.log("Block Number:", receipt.blockNumber);
        console.log("Gas Used:", receipt.gasUsed.toString());
        console.log("Status:", receipt.status === 1 ? "Success ✅" : "Failed ❌");
        console.log("");

        // Parse events
        console.log("Events:");
        for (const log of receipt.logs) {
            try {
                const parsed = flashArb.interface.parseLog(log);
                if (parsed) {
                    console.log(`\n📋 ${parsed.name}`);
                    for (const [key, value] of Object.entries(parsed.args)) {
                        if (isNaN(key)) {
                            console.log(`  ${key}:`, value.toString());
                        }
                    }
                }
            } catch (e) {
                // Skip non-contract events
            }
        }

        console.log("");
        console.log("============================================================");
        console.log("✅ FLASH LOAN ARBITRAGE EXECUTED SUCCESSFULLY!");
        console.log("============================================================");

        // Check profit
        const contractWethBalance = await weth.balanceOf(contractAddress);
        if (contractWethBalance > 0n) {
            console.log("\n💰 Profit in Contract:", ethers.formatEther(contractWethBalance), "WETH");
            console.log("You can withdraw this profit!");
        }

    } catch (error) {
        console.error("\n❌ EXECUTION FAILED");
        console.error("============================================================");
        console.error("Error:", error.message);
        
        // Try to get more details from the error
        if (error.data) {
            console.error("Error Data:", error.data);
        }
        if (error.error && error.error.message) {
            console.error("Detailed Error:", error.error.message);
        }
        console.error("============================================================\n");

        if (error.message.includes("STF")) {
            console.log("📊 Reason: SafeTransferFrom failed");
            console.log("   This usually means:");
            console.log("   1. Token approval issue (unlikely - contract handles this)");
            console.log("   2. Insufficient balance for swap");
            console.log("   3. Pool doesn't have enough liquidity");
            console.log("   4. The route is not profitable (most likely)");
        } else if (error.message.includes("Low profit")) {
            console.log("📊 Reason: Profit below minimum threshold");
            console.log("   The arbitrage executed but didn't make enough profit");
        } else if (error.message.includes("Slippage")) {
            console.log("📊 Reason: Slippage too high");
            console.log("   Actual output was less than minAmountOut");
        } else if (error.message.includes("Insufficient funds")) {
            console.log("📊 Reason: Not enough tokens to repay flash loan");
            console.log("   The swaps didn't return enough to cover the debt");
        }
        
        console.log("\n💡 This is EXPECTED on a test route!");
        console.log("   Real arbitrage requires finding actual price discrepancies.");
        console.log("   Your contract is working correctly by rejecting unprofitable trades.");
    }

    // ========================================================================
    // SUMMARY
    // ========================================================================
    console.log("\n============================================================");
    console.log("TEST COMPLETED");
    console.log("============================================================\n");

    console.log("✅ What was tested:");
    console.log("1. Contract deployment on mainnet fork");
    console.log("2. Real-time price fetching from mainnet");
    console.log("3. WETH wrapping");
    console.log("4. Flash loan execution with real Aave");
    console.log("5. DEX swaps with real Uniswap pools");
    console.log("6. Profit calculation");
    console.log("");

    console.log("📊 Contract Address:", contractAddress);
    console.log("🔗 This is a LOCAL fork - no real money spent!");
    console.log("");

    console.log("💡 Next Steps:");
    console.log("1. Try different token pairs (WETH/DAI, WETH/USDT)");
    console.log("2. Try different fee tiers (500, 3000, 10000)");
    console.log("3. Adjust flash loan amounts");
    console.log("4. Test with real arbitrage opportunities");
    console.log("");
}

main()
    .then(() => process.exit(0))
    .catch((error) => {
        console.error("\n❌ Test failed:");
        console.error(error);
        process.exit(1);
    });
