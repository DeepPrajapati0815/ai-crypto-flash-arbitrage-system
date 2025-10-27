// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

/**
 * @title IFlashArbProductionSafe
 * @notice Interface for production-safe flash arbitrage contract
 * @dev Defines the interface that Rust bindings expect
 */
interface IFlashArbProductionSafe {
    // Events
    event ArbitrageExecuted(
        address indexed token,
        uint256 amountBorrowed,
        uint256 amountReturned,
        uint256 profit,
        uint256 gasUsed
    );
    
    event ArbitrageFailed(
        address indexed token,
        uint256 amountBorrowed,
        string reason
    );
    
    event EmergencyPause(string reason);
    
    event CircuitBreakerTriggered(uint256 failedAttempts);

    // Main execution method (Rust-compatible)
    function executeFlashArbSafe(
        address asset,
        uint256 amount,
        uint256 minProfitWei,
        uint16 maxSlippageBps,
        bytes calldata params
    ) external returns (uint256 profit);

    // Aave callback
    function executeOperation(
        address asset,
        uint256 amount,
        uint256 premium,
        address initiator,
        bytes calldata params
    ) external returns (bool);

    // Admin functions
    function authorizeBundleSubmitter(address submitter) external;
    function revokeBundleSubmitter(address submitter) external;
    function setBundleOnlyMode(bool enabled) external;
    function emergencyPause(string calldata reason) external;
    function emergencyUnpause() external;

    // View functions
    function getMinProfitWei() external view returns (uint256);
    function getFailedAttempts() external view returns (uint256);
    function isEmergencyPaused() external view returns (bool);
    function bundleOnlyMode() external view returns (bool);
    function authorizedBundleSubmitters(address) external view returns (bool);
}

/**
 * @title IFlashArbSecure
 * @notice Interface for secure flash arbitrage contract with commit-reveal
 * @dev Defines the interface for MEV-protected arbitrage execution
 */
interface IFlashArbSecure {
    // Events
    event RouteCommitted(bytes32 indexed routeHash, address indexed executor, uint256 blockNumber);
    event RouteRevealed(bytes32 indexed routeHash, address indexed executor, uint256 profit, uint256 gasPrice);

    // Commit-reveal methods
    function commitRoute(bytes32 routeHash) external;
    function revealRoute(
        bytes32 routeHash,
        address asset,
        uint256 amount,
        TradeRoute[] calldata routes,
        uint256 nonce,
        bytes calldata signature
    ) external;

    // Secure execution
    function executeArbitrageSecure(
        address asset,
        uint256 amount,
        TradeRoute[] calldata routes,
        uint256 nonce,
        bytes calldata signature
    ) external;

    // View functions
    function routeCommitmentBlocks(bytes32) external view returns (uint256);
    function routeExecuted(bytes32) external view returns (bool);
    function isRouteExecutable(bytes32 routeHash) external view returns (bool);
}

/**
 * @title IFlashArbBase
 * @notice Interface for base flash arbitrage contract
 * @dev Compatible with Rust bindings using tuple parameters
 */
interface IFlashArbBase {
    // Main execution method (Rust-compatible with tuples)
    function executeFlashArbitrage(
        address asset,
        uint256 amount,
        (uint8, address, address, uint32, uint256, uint256)[] calldata tuples
    ) external;

    // Aave callback
    function executeOperation(
        address asset,
        uint256 amount,
        uint256 premium,
        address initiator,
        bytes calldata params
    ) external returns (bool);

    // Admin functions
    function authorizeExecutor(address executor) external;
    function revokeExecutor(address executor) external;
    function pause() external;
    function unpause() external;
    function emergencyPause() external;
    function emergencyUnpause() external;

    // View functions
    function owner() external view returns (address);
    function paused() external view returns (bool);
    function emergencyPaused() external view returns (bool);
    function authorizedExecutors(address) external view returns (bool);
    function minProfitBps() external view returns (uint256);
    function maxGasPrice() external view returns (uint256);
}

// TradeRoute struct definition for interfaces
struct TradeRoute {
    DexType dexType;
    address tokenIn;
    address tokenOut;
    uint32 poolFee;
    uint256 amountIn;
    uint256 minAmountOut;
    uint256 deadline;
    uint16 maxSlippageBps;
    bytes32 routeHash;
}

enum DexType {
    UniswapV3,
    Sushiswap
}
