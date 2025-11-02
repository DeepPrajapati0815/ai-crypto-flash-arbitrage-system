const { ethers } = require("hardhat");

async function main() {
  console.log("🚀 Starting deployment to Sepolia testnet...\n");

  // Get the deployer account
  const [deployer] = await ethers.getSigners();
  console.log("Deploying contracts with account:", deployer.address);
  const balance = await ethers.provider.getBalance(deployer.address);
  console.log("Account balance:", ethers.formatEther(balance), "ETH\n");

  // ⚠️  IMPORTANT: Verify these addresses before deployment!
  // These addresses may not be current. Please check official documentation:
  // - Aave V3: https://docs.aave.com/developers/deployed-contracts/v3-testnets
  // - Uniswap V3: https://docs.uniswap.org/contracts/v3/reference/deployments
  // - Sushiswap: Check their official documentation
  
  const SEPOLIA_ADDRESSES = {
    // Aave V3 Sepolia - VERIFY THIS ADDRESS!
    aavePool: "0x012bAC54348C0E635dCAc9D5FB99f06F24136C9A",
    weth: "0x387d311e47e80b498169e6fb51d3193167d89F7D",
    
    // Uniswap V3 Sepolia - VERIFY THIS ADDRESS!
    uniswapV3Router: "0x3bFA4769FB09eefC5a80d6E87c3B9C650f7Ae48E",
    uniswapV3Factory: "0x0227628f3F023bb0B980b67D528571c95c6DaC1c",
    
    // Sushiswap Sepolia - VERIFY THIS ADDRESS!
    sushiswapRouter: "0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506",
  };

  console.log("📋 Using Sepolia testnet addresses:");
  console.log("Aave Pool:", SEPOLIA_ADDRESSES.aavePool);
  console.log("WETH:", SEPOLIA_ADDRESSES.weth);
  console.log("Uniswap V3 Router:", SEPOLIA_ADDRESSES.uniswapV3Router);
  console.log("Sushiswap Router:", SEPOLIA_ADDRESSES.sushiswapRouter);
  console.log("");

  try {
    // Deploy FlashArbUltimate (inherits from FlashArb, so base is included)
    console.log("📦 Deploying FlashArbUltimate...");
    const FlashArbUltimate = await ethers.getContractFactory("FlashArbUltimate");
    const flashArbUltimate = await FlashArbUltimate.deploy(
      SEPOLIA_ADDRESSES.aavePool,
      SEPOLIA_ADDRESSES.uniswapV3Router,
      SEPOLIA_ADDRESSES.sushiswapRouter,
      ethers.parseEther("0.001"), // _minProfitWei = 0.001 ETH
      5 // _maxFailedAttempts = 5
    );
    await flashArbUltimate.waitForDeployment();
    const flashArbUltimateAddress = await flashArbUltimate.getAddress();
    console.log("✅ FlashArbUltimate deployed to:", flashArbUltimateAddress);

    console.log("\n🎉 All contracts deployed successfully!");
    console.log("\n📋 Deployment Summary:");
    console.log("========================");
    console.log("FlashArbUltimate:", flashArbUltimateAddress);
    console.log("\n🔗 View on Etherscan:");
    console.log(`https://sepolia.etherscan.io/address/${flashArbUltimateAddress}`);

    // Save deployment info
    const deploymentInfo = {
      network: "sepolia",
      timestamp: new Date().toISOString(),
      deployer: deployer.address,
      contracts: {
        FlashArbUltimate: flashArbUltimateAddress,
      },
      addresses: SEPOLIA_ADDRESSES,
    };

    const fs = require("fs");
    fs.writeFileSync(
      "deployments/sepolia-deployment.json",
      JSON.stringify(deploymentInfo, null, 2)
    );
    console.log("\n💾 Deployment info saved to deployments/sepolia-deployment.json");

  } catch (error) {
    console.error("❌ Deployment failed:", error);
    process.exit(1);
  }
}

main()
  .then(() => process.exit(0))
  .catch((error) => {
    console.error(error);
    process.exit(1);
  });