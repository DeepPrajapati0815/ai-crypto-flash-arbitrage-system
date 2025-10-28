const { run } = require("hardhat");

async function main() {
  console.log("🔍 Starting contract verification on Sepolia...\n");

  // Read deployment info
  const fs = require("fs");
  const deploymentInfo = JSON.parse(fs.readFileSync("deployments/sepolia-deployment.json", "utf8"));

  const contracts = deploymentInfo.contracts;
  const constructorArgs = {
    FlashArb: [
      deploymentInfo.addresses.aavePool,
      deploymentInfo.addresses.uniswapV3Router,
      deploymentInfo.addresses.sushiswapRouter,
    ],
    FlashArbSecure: [
      deploymentInfo.addresses.aavePool,
      deploymentInfo.addresses.uniswapV3Router,
      deploymentInfo.addresses.sushiswapRouter,
    ],
    FlashArbWithTimelock: [
      deploymentInfo.addresses.aavePool,
      deploymentInfo.addresses.uniswapV3Router,
      deploymentInfo.addresses.sushiswapRouter,
    ],
    FlashArbOptimized: [
      deploymentInfo.addresses.aavePool,
      deploymentInfo.addresses.uniswapV3Router,
      deploymentInfo.addresses.sushiswapRouter,
      "0x0000000000000000000000000000000000000000", // permit2
    ],
    FlashArbProductionSafe: [
      deploymentInfo.addresses.aavePool,
      deploymentInfo.addresses.uniswapV3Router,
      deploymentInfo.addresses.sushiswapRouter,
      "1000000000000000", // 0.001 ETH in wei
    ],
  };

  for (const [contractName, address] of Object.entries(contracts)) {
    try {
      console.log(`🔍 Verifying ${contractName} at ${address}...`);
      
      await run("verify:verify", {
        address: address,
        constructorArguments: constructorArgs[contractName],
      });
      
      console.log(`✅ ${contractName} verified successfully!`);
      console.log(`🔗 https://sepolia.etherscan.io/address/${address}\n`);
      
    } catch (error) {
      if (error.message.includes("Already Verified")) {
        console.log(`⚠️  ${contractName} already verified\n`);
      } else {
        console.error(`❌ Failed to verify ${contractName}:`, error.message);
      }
    }
  }

  console.log("🎉 Verification process completed!");
}

main()
  .then(() => process.exit(0))
  .catch((error) => {
    console.error(error);
    process.exit(1);
  });
