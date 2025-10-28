const { ethers } = require("hardhat");

async function main() {
  console.log("🚀 Deploying with custom Sepolia addresses...\n");

  // Get the deployer account
  const [deployer] = await ethers.getSigners();
  console.log("Deploying contracts with account:", deployer.address);
  console.log("Account balance:", ethers.utils.formatEther(await deployer.getBalance()), "ETH\n");

  // ⚠️  UPDATE THESE ADDRESSES WITH CURRENT ONES FROM OFFICIAL DOCS
  const SEPOLIA_ADDRESSES = {
    // Get these from official Aave V3 documentation
    aavePool: process.env.AAVE_POOL_ADDRESS || "0x6Ae43d3271ff6888e7Fc43Fd7321a503ff738951",
    weth: process.env.WETH_ADDRESS || "0xfFf9976782d46CC05630D1f6eBAb18b2324d6B14",
    
    // Get these from official Uniswap V3 documentation
    uniswapV3Router: process.env.UNISWAP_V3_ROUTER || "0x3bFA4769FB09eefC5a80d6E87c3B9C650f7Ae48E",
    uniswapV3Factory: process.env.UNISWAP_V3_FACTORY || "0x0227628f3F023bb0B980b67D528571c95c6DaC1c",
    
    // Get this from official Sushiswap documentation
    sushiswapRouter: process.env.SUSHISWAP_ROUTER || "0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506",
  };

  console.log("📋 Using addresses (update these in .env or here):");
  console.log("Aave Pool:", SEPOLIA_ADDRESSES.aavePool);
  console.log("WETH:", SEPOLIA_ADDRESSES.weth);
  console.log("Uniswap V3 Router:", SEPOLIA_ADDRESSES.uniswapV3Router);
  console.log("Sushiswap Router:", SEPOLIA_ADDRESSES.sushiswapRouter);
  console.log("");

  // Verify addresses before deployment
  console.log("🔍 Verifying addresses...");
  for (const [name, address] of Object.entries(SEPOLIA_ADDRESSES)) {
    const code = await ethers.provider.getCode(address);
    if (code === "0x") {
      console.log(`❌ ${name}: No contract found at ${address}`);
      console.log(`   Please update this address in your .env file or this script`);
    } else {
      console.log(`✅ ${name}: Contract found`);
    }
  }
  console.log("");

  try {
    // Deploy FlashArb (base contract)
    console.log("📦 Deploying FlashArb...");
    const FlashArb = await ethers.getContractFactory("FlashArb");
    const flashArb = await FlashArb.deploy(
      SEPOLIA_ADDRESSES.aavePool,
      SEPOLIA_ADDRESSES.uniswapV3Router,
      SEPOLIA_ADDRESSES.sushiswapRouter
    );
    await flashArb.deployed();
    console.log("✅ FlashArb deployed to:", flashArb.address);

    // Deploy FlashArbSecure
    console.log("📦 Deploying FlashArbSecure...");
    const FlashArbSecure = await ethers.getContractFactory("FlashArbSecure");
    const flashArbSecure = await FlashArbSecure.deploy(
      SEPOLIA_ADDRESSES.aavePool,
      SEPOLIA_ADDRESSES.uniswapV3Router,
      SEPOLIA_ADDRESSES.sushiswapRouter
    );
    await flashArbSecure.deployed();
    console.log("✅ FlashArbSecure deployed to:", flashArbSecure.address);

    console.log("\n🎉 Deployment completed!");
    console.log("\n📋 Deployment Summary:");
    console.log("========================");
    console.log("FlashArb:", flashArb.address);
    console.log("FlashArbSecure:", flashArbSecure.address);

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
