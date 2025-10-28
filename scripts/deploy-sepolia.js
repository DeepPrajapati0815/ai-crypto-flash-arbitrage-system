const { ethers } = require("hardhat");

async function main() {
  console.log("🚀 Starting deployment to Sepolia testnet...\n");

  // Get the deployer account
  const [deployer] = await ethers.getSigners();
  console.log("Deploying contracts with account:", deployer.address);
  console.log("Account balance:", ethers.utils.formatEther(await deployer.getBalance()), "ETH\n");

  // ⚠️  IMPORTANT: Verify these addresses before deployment!
  // These addresses may not be current. Please check official documentation:
  // - Aave V3: https://docs.aave.com/developers/deployed-contracts/v3-testnets
  // - Uniswap V3: https://docs.uniswap.org/contracts/v3/reference/deployments
  // - Sushiswap: Check their official documentation
  
  const SEPOLIA_ADDRESSES = {
    // Aave V3 Sepolia - VERIFY THIS ADDRESS!
    aavePool: "0x6Ae43d3271ff6888e7Fc43Fd7321a503ff738951",
    weth: "0xfFf9976782d46CC05630D1f6eBAb18b2324d6B14",
    
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

    // Deploy FlashArbWithTimelock
    console.log("📦 Deploying FlashArbWithTimelock...");
    const FlashArbWithTimelock = await ethers.getContractFactory("FlashArbWithTimelock");
    const flashArbWithTimelock = await FlashArbWithTimelock.deploy(
      SEPOLIA_ADDRESSES.aavePool,
      SEPOLIA_ADDRESSES.uniswapV3Router,
      SEPOLIA_ADDRESSES.sushiswapRouter
    );
    await flashArbWithTimelock.deployed();
    console.log("✅ FlashArbWithTimelock deployed to:", flashArbWithTimelock.address);

    // Deploy FlashArbOptimized
    console.log("📦 Deploying FlashArbOptimized...");
    const FlashArbOptimized = await ethers.getContractFactory("FlashArbOptimized");
    const flashArbOptimized = await FlashArbOptimized.deploy(
      SEPOLIA_ADDRESSES.aavePool,
      SEPOLIA_ADDRESSES.uniswapV3Router,
      SEPOLIA_ADDRESSES.sushiswapRouter,
      ethers.constants.AddressZero // permit2 not needed for testnet
    );
    await flashArbOptimized.deployed();
    console.log("✅ FlashArbOptimized deployed to:", flashArbOptimized.address);

    // Deploy FlashArbProductionSafe
    console.log("📦 Deploying FlashArbProductionSafe...");
    const FlashArbProductionSafe = await ethers.getContractFactory("FlashArbProductionSafe");
    const flashArbProductionSafe = await FlashArbProductionSafe.deploy(
      SEPOLIA_ADDRESSES.aavePool,
      SEPOLIA_ADDRESSES.uniswapV3Router,
      SEPOLIA_ADDRESSES.sushiswapRouter,
      ethers.utils.parseEther("0.001") // MIN_PROFIT_WEI = 0.001 ETH
    );
    await flashArbProductionSafe.deployed();
    console.log("✅ FlashArbProductionSafe deployed to:", flashArbProductionSafe.address);

    console.log("\n🎉 All contracts deployed successfully!");
    console.log("\n📋 Deployment Summary:");
    console.log("========================");
    console.log("FlashArb:", flashArb.address);
    console.log("FlashArbSecure:", flashArbSecure.address);
    console.log("FlashArbWithTimelock:", flashArbWithTimelock.address);
    console.log("FlashArbOptimized:", flashArbOptimized.address);
    console.log("FlashArbProductionSafe:", flashArbProductionSafe.address);
    console.log("\n🔗 View on Etherscan:");
    console.log(`https://sepolia.etherscan.io/address/${flashArb.address}`);
    console.log(`https://sepolia.etherscan.io/address/${flashArbSecure.address}`);
    console.log(`https://sepolia.etherscan.io/address/${flashArbWithTimelock.address}`);
    console.log(`https://sepolia.etherscan.io/address/${flashArbOptimized.address}`);
    console.log(`https://sepolia.etherscan.io/address/${flashArbProductionSafe.address}`);

    // Save deployment info
    const deploymentInfo = {
      network: "sepolia",
      timestamp: new Date().toISOString(),
      deployer: deployer.address,
      contracts: {
        FlashArb: flashArb.address,
        FlashArbSecure: flashArbSecure.address,
        FlashArbWithTimelock: flashArbWithTimelock.address,
        FlashArbOptimized: flashArbOptimized.address,
        FlashArbProductionSafe: flashArbProductionSafe.address,
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
