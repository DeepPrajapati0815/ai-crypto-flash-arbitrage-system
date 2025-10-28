const { ethers } = require("hardhat");

async function main() {
  console.log("🔍 Verifying Sepolia testnet addresses...\n");

  // Addresses to verify
  const addresses = {
    "Aave V3 Pool": "0x6Ae43d3271ff6888e7Fc43Fd7321a503ff738951",
    "WETH": "0xfFf9976782d46CC05630D1f6eBAb18b2324d6B14",
    "Uniswap V3 Router": "0x3bFA4769FB09eefC5a80d6E87c3B9C650f7Ae48E",
    "Uniswap V3 Factory": "0x0227628f3F023bb0B980b67D528571c95c6DaC1c",
    "Sushiswap Router": "0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506",
  };

  console.log("📋 Checking addresses on Sepolia testnet:\n");

  for (const [name, address] of Object.entries(addresses)) {
    try {
      console.log(`🔍 Checking ${name}: ${address}`);
      
      // Check if address has code
      const code = await ethers.provider.getCode(address);
      
      if (code === "0x") {
        console.log(`❌ ${name}: No contract found at this address`);
      } else {
        console.log(`✅ ${name}: Contract found (${code.length} bytes of code)`);
        
        // Try to get some basic info
        try {
          const balance = await ethers.provider.getBalance(address);
          console.log(`   Balance: ${ethers.utils.formatEther(balance)} ETH`);
        } catch (e) {
          // Not an EOA, that's expected for contracts
        }
      }
      
      console.log(`   Etherscan: https://sepolia.etherscan.io/address/${address}\n`);
      
    } catch (error) {
      console.log(`❌ ${name}: Error checking address - ${error.message}\n`);
    }
  }

  console.log("📚 Official Documentation Links:");
  console.log("=================================");
  console.log("Aave V3 Testnets: https://docs.aave.com/developers/deployed-contracts/v3-testnets");
  console.log("Uniswap V3 Deployments: https://docs.uniswap.org/contracts/v3/reference/deployments");
  console.log("Sushiswap: Check their official documentation");
  console.log("\n⚠️  IMPORTANT: Always verify addresses from official sources before deployment!");
}

main()
  .then(() => process.exit(0))
  .catch((error) => {
    console.error(error);
    process.exit(1);
  });
