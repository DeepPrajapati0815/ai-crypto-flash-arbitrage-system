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
    // Deploy FlashArb (base contract)
    console.log("📦 Deploying FlashArb...");
    // const FlashArb = await ethers.getContractFactory("FlashArb");
    // const flashArb = await FlashArb.deploy(
    //   SEPOLIA_ADDRESSES.aavePool,
    //   SEPOLIA_ADDRESSES.uniswapV3Router,
    //   SEPOLIA_ADDRESSES.sushiswapRouter
    // );
    // await flashArb.waitForDeployment();
    // const flashArbAddress = await flashArb.getAddress();
    // console.log("✅ FlashArb deployed to:", flashArbAddress);

    // Deploy FlashArbSecure
    console.log("📦 Deploying FlashArbSecure...");
    const FlashArbSecure = await ethers.getContractFactory("FlashArbSecure");
    // const flashArbSecure = await FlashArbSecure.deploy(
    //   SEPOLIA_ADDRESSES.aavePool,
    //   SEPOLIA_ADDRESSES.uniswapV3Router,
    //   SEPOLIA_ADDRESSES.sushiswapRouter
    // );
    // await flashArbSecure.waitForDeployment();
    // const flashArbSecureAddress = await flashArbSecure.getAddress();
    // console.log("✅ FlashArbSecure deployed to:", flashArbSecureAddress);

    // Deploy FlashArbWithTimelock
    // console.log("📦 Deploying FlashArbWithTimelock...");
    // const FlashArbWithTimelock = await ethers.getContractFactory("FlashArbWithTimelock");
    // const flashArbWithTimelock = await FlashArbWithTimelock.deploy(
    //   SEPOLIA_ADDRESSES.aavePool,
    //   SEPOLIA_ADDRESSES.uniswapV3Router,
    //   SEPOLIA_ADDRESSES.sushiswapRouter
    // );
    // await flashArbWithTimelock.waitForDeployment();
    // const flashArbWithTimelockAddress = await flashArbWithTimelock.getAddress();
    // console.log("✅ FlashArbWithTimelock deployed to:", flashArbWithTimelockAddress);

    // Deploy FlashArbOptimized
    // console.log("📦 Deploying FlashArbOptimized...");
    // const FlashArbOptimized = await ethers.getContractFactory("FlashArbOptimized");
    // const flashArbOptimized = await FlashArbOptimized.deploy(
    //   SEPOLIA_ADDRESSES.aavePool,
    //   SEPOLIA_ADDRESSES.uniswapV3Router,
    //   SEPOLIA_ADDRESSES.sushiswapRouter,
    //   ethers.ZeroAddress // permit2 not needed for testnet
    // );
    // await flashArbOptimized.waitForDeployment();
    // const flashArbOptimizedAddress = await flashArbOptimized.getAddress();
    // console.log("✅ FlashArbOptimized deployed to:", flashArbOptimizedAddress);

    // Deploy FlashArbProductionSafe
    console.log("📦 Deploying FlashArbProductionSafe...");
    const FlashArbProductionSafe = await ethers.getContractFactory("FlashArbProductionSafe");
    const flashArbProductionSafe = await FlashArbProductionSafe.deploy(
      SEPOLIA_ADDRESSES.aavePool,
      SEPOLIA_ADDRESSES.uniswapV3Router,
    SEPOLIA_ADDRESSES.sushiswapRouter,
    ethers.ZeroAddress,
    ethers.parseEther("0.001") // MIN_PROFIT_WEI = 0.001 ETH
    );
    await flashArbProductionSafe.waitForDeployment();
    const flashArbProductionSafeAddress = await flashArbProductionSafe.getAddress();
    console.log("✅ FlashArbProductionSafe deployed to:", flashArbProductionSafeAddress);

    console.log("\n🎉 All contracts deployed successfully!");
    console.log("\n📋 Deployment Summary:");
    console.log("========================");
  // console.log("FlashArb:", flashArbAddress);
  // console.log("FlashArbSecure:", flashArbSecureAddress);
  // console.log("FlashArbWithTimelock:", flashArbWithTimelockAddress);
  // console.log("FlashArbOptimized:", flashArbOptimizedAddress);
    console.log("FlashArbProductionSafe:", flashArbProductionSafeAddress);
    console.log("\n🔗 View on Etherscan:");
  // console.log(`https://sepolia.etherscan.io/address/${flashArbAddress}`);
  // console.log(`https://sepolia.etherscan.io/address/${flashArbSecureAddress}`);
  // console.log(`https://sepolia.etherscan.io/address/${flashArbWithTimelockAddress}`);
  // console.log(`https://sepolia.etherscan.io/address/${flashArbOptimizedAddress}`);
    console.log(`https://sepolia.etherscan.io/address/${flashArbProductionSafeAddress}`);

    // Save deployment info
    const deploymentInfo = {
      network: "sepolia",
      timestamp: new Date().toISOString(),
      deployer: deployer.address,
      contracts: {
      // FlashArb: flashArbAddress,
      // FlashArbSecure: flashArbSecureAddress,
      // FlashArbWithTimelock: flashArbWithTimelockAddress,
      // FlashArbOptimized: flashArbOptimizedAddress,
        FlashArbProductionSafe: flashArbProductionSafeAddress,
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
