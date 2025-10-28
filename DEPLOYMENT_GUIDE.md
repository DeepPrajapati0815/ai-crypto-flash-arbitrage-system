# 🚀 Sepolia Testnet Deployment Guide

## Prerequisites

### 1. Environment Setup
Create a `.env` file in your project root with:

```bash
# Sepolia RPC URL (choose one provider)
SEPOLIA_RPC_URL=https://sepolia.infura.io/v3/YOUR_PROJECT_ID
# OR
# SEPOLIA_RPC_URL=https://eth-sepolia.g.alchemy.com/v2/YOUR_API_KEY
# OR
# SEPOLIA_RPC_URL=https://rpc.sepolia.org

# Private Key (without 0x prefix)
EVM_PRIVATE_KEY=your_private_key_here

# Etherscan API Key (for contract verification)
ETHERSCAN_API_KEY=your_etherscan_api_key_here
```

### 2. Get Sepolia ETH
You need Sepolia testnet ETH for gas fees:
- **Faucet**: https://sepoliafaucet.com/
- **Alchemy Faucet**: https://sepoliafaucet.com/
- **Chainlink Faucet**: https://faucets.chain.link/sepolia

### 3. Required Accounts & API Keys

#### RPC Provider (choose one):
- **Infura**: https://infura.io/ (free tier available)
- **Alchemy**: https://alchemy.com/ (free tier available)
- **Public RPC**: https://rpc.sepolia.org (free, but less reliable)

#### Etherscan API Key:
- **Etherscan**: https://etherscan.io/apis (free)

## Deployment Steps

### 1. Install Dependencies
```bash
npm install
```

### 2. Compile Contracts
```bash
npx hardhat compile
```

### 3. Deploy to Sepolia
```bash
npx hardhat run scripts/deploy-sepolia.js --network sepolia
```

### 4. Verify Contracts
```bash
npx hardhat run scripts/verify-sepolia.js --network sepolia
```

## Contract Addresses (Sepolia Testnet)

The deployment script uses these verified Sepolia addresses:

- **Aave V3 Pool**: `0x6Ae43d3271ff6888e7Fc43Fd7321a503ff738951`
- **WETH**: `0xfFf9976782d46CC05630D1f6eBAb18b2324d6B14`
- **Uniswap V3 Router**: `0x3bFA4769FB09eefC5a80d6E87c3B9C650f7Ae48E`
- **Sushiswap Router**: `0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506`

## Deployed Contracts

After successful deployment, you'll get:

1. **FlashArb** - Base arbitrage contract
2. **FlashArbSecure** - With MEV protection and oracle validation
3. **FlashArbWithTimelock** - With timelock for parameter changes
4. **FlashArbOptimized** - Gas-optimized version
5. **FlashArbProductionSafe** - Production-ready with comprehensive safety checks

## Post-Deployment

### 1. Check Deployment
- View contracts on Etherscan: https://sepolia.etherscan.io/
- Check deployment info in `deployments/sepolia-deployment.json`

### 2. Test Contracts
- Use the deployed contract addresses in your Rust integration
- Test with small amounts first
- Monitor gas usage and transaction success

### 3. Security Considerations
- Never commit your `.env` file
- Use a dedicated testnet wallet
- Keep your private key secure
- Test thoroughly before mainnet deployment

## Troubleshooting

### Common Issues:

1. **"Insufficient funds"**: Get more Sepolia ETH from faucets
2. **"Gas price too low"**: Increase gas price in hardhat.config.js
3. **"Contract verification failed"**: Check constructor arguments match
4. **"RPC error"**: Try a different RPC provider

### Gas Optimization:
- Contracts are optimized for deployment size
- Gas costs should be reasonable on testnet
- Monitor gas usage during testing

## Next Steps

1. **Integration Testing**: Connect your Rust backend to deployed contracts
2. **Security Audit**: Consider professional audit before mainnet
3. **Mainnet Deployment**: Use similar process for mainnet deployment
4. **Monitoring**: Set up monitoring and alerting for production use

## Support

If you encounter issues:
1. Check the deployment logs
2. Verify your environment variables
3. Ensure you have sufficient Sepolia ETH
4. Check network connectivity to RPC provider
