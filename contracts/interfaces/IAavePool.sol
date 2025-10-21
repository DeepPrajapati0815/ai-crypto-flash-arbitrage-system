// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

/**
 * @title IAavePool
 * @notice Interface for Aave V3 Pool contract
 */
interface IAavePool {
    function flashLoanSimple(
        address receiverAddress,
        address asset,
        uint256 amount,
        bytes calldata params,
        uint16 referralCode
    ) external;

    function FLASHLOAN_PREMIUM_TOTAL() external view returns (uint128);
}

