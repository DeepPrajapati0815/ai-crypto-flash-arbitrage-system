// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

/**
 * @title FlashArbErrors
 * @notice ✅ ISSUE #3 FIX: Custom errors for gas optimization (saves 15-20k gas per transaction)
 * @dev Custom errors are ~10x cheaper than require strings
 */

// ============ Validation Errors ============

/// @notice Invalid route hash provided
error InvalidRouteHash();

/// @notice Route already committed
error RouteAlreadyCommitted();

/// @notice Route not committed
error RouteNotCommitted();

/// @notice Route already executed
error RouteAlreadyExecuted();

/// @notice Commit delay not met (MEV protection)
error CommitDelayNotMet(uint256 elapsed, uint256 required);

/// @notice Commitment expired
error CommitmentExpired(uint256 elapsed, uint256 maximum);

/// @notice Route hash mismatch
error RouteHashMismatch(bytes32 expected, bytes32 actual);

/// @notice Nonce already used (replay attack prevention)
error NonceAlreadyUsed(address executor, uint256 nonce);

// ============ Input Validation Errors ============

/// @notice Invalid token address
error InvalidToken(address token);

/// @notice Invalid amount
error InvalidAmount(uint256 amount);

/// @notice Insufficient output amount
error InsufficientOutput(uint256 expected, uint256 actual);

/// @notice Route array too large
error RouteTooLarge(uint256 length, uint256 maximum);

/// @notice Empty route array
error EmptyRoute();

/// @notice Mismatched route amounts
error RouteAmountMismatch(uint256 routeIndex, uint256 expected, uint256 actual);

// ============ Execution Errors ============

/// @notice Gas price too high
error GasPriceTooHigh(uint256 actual, uint256 maximum);

/// @notice Trade execution failed
error TradeExecutionFailed(uint256 routeIndex);

/// @notice Flash loan repayment failed
error FlashLoanRepaymentFailed(uint256 amountOwed, uint256 balance);

/// @notice Insufficient profit
error InsufficientProfit(uint256 profit, uint256 minimum);

/// @notice DEX not supported
error UnsupportedDEX(uint8 dexType);

/// @notice Slippage exceeded
error SlippageExceeded(uint256 expected, uint256 actual, uint256 maxSlippageBps);

// ============ Security Errors ============

/// @notice Not authorized
error NotAuthorized(address caller);

/// @notice Chain ID mismatch (fork detection)
error ChainIdMismatch(uint256 expected, uint256 actual);

/// @notice Deadline exceeded
error DeadlineExceeded(uint256 deadline, uint256 currentTime);

/// @notice Contract paused
error ContractPaused();

/// @notice Contract not paused
error ContractNotPaused();

// ============ Transfer Errors ============

/// @notice Token transfer failed
error TokenTransferFailed(address token, address from, address to, uint256 amount);

/// @notice Token approval failed
error TokenApprovalFailed(address token, address spender, uint256 amount);

/// @notice Insufficient balance
error InsufficientBalance(address token, uint256 required, uint256 available);

