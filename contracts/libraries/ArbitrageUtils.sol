// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

/**
 * @title ArbitrageUtils
 * @notice Library for arbitrage calculations and validations
 * @dev Extracted to reduce main contract size
 */
library ArbitrageUtils {
    uint256 private constant BPS_BASE = 10000;
    uint256 private constant MAX_SLIPPAGE_BPS = 200; // 2%

    /**
     * @notice Check slippage protection with overflow safety
     * @param expectedAmount Expected minimum amount
     * @param actualAmount Actual amount received
     * @param maxSlippageBps Maximum slippage in basis points
     * @return slippage The calculated slippage in basis points
     */
    function checkSlippage(
        uint256 expectedAmount,
        uint256 actualAmount,
        uint256 maxSlippageBps
    ) internal pure returns (uint256 slippage) {
        require(expectedAmount > 0, "Invalid expected amount");
        require(actualAmount > 0, "Invalid actual amount");
        
        if (actualAmount < expectedAmount) {
            uint256 difference = expectedAmount - actualAmount;
            require(difference <= type(uint256).max / BPS_BASE, "Amount too large for slippage calc");
            slippage = (difference * BPS_BASE) / expectedAmount;
            require(slippage <= maxSlippageBps, "Slippage exceeded");
            require(slippage <= MAX_SLIPPAGE_BPS, "Slippage exceeds maximum allowed");
        }
    }

    /**
     * @notice Calculate minimum profit threshold
     * @param amount Amount to calculate profit for
     * @param minProfitBps Minimum profit in basis points
     * @return Minimum profit required
     */
    function calculateMinProfit(uint256 amount, uint256 minProfitBps) internal pure returns (uint256) {
        return (amount * minProfitBps) / BPS_BASE;
    }

    /**
     * @notice Generate route hash for continuity tracking
     * @param tokenIn Input token
     * @param tokenOut Output token
     * @param amountIn Input amount
     * @param minAmountOut Minimum output amount
     * @param timestamp Current timestamp
     * @return Route hash
     */
    function generateRouteHash(
        address tokenIn,
        address tokenOut,
        uint256 amountIn,
        uint256 minAmountOut,
        uint256 timestamp
    ) internal pure returns (bytes32) {
        return keccak256(abi.encodePacked(tokenIn, tokenOut, amountIn, minAmountOut, timestamp));
    }

    /**
     * @notice Validate route deadline
     * @param deadline Route deadline
     * @param currentTime Current block timestamp
     * @return True if deadline is valid
     */
    function isValidDeadline(uint256 deadline, uint256 currentTime) internal pure returns (bool) {
        return deadline > currentTime && deadline <= currentTime + 300; // Max 5 min ahead
    }
}
