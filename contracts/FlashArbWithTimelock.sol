// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

import "./FlashArb.sol";

/**
 * @title FlashArbWithTimelock
 * @notice Enhanced FlashArb with timelock mechanism for parameter changes
 * @dev Adds 2-day timelock delay for critical parameter changes to prevent
 *      malicious or accidental instant changes that could harm users
 */
contract FlashArbWithTimelock is FlashArb {
    // Timelock configuration
    uint256 public constant TIMELOCK_DELAY = 2 days;
    uint256 public constant MIN_TIMELOCK_DELAY = 1 days;
    uint256 public constant MAX_TIMELOCK_DELAY = 7 days;
    
    // Import constants from parent
    uint256 private constant MAX_SLIPPAGE_BPS = 200; // 2%
    
    // Required confirmations (from parent contract)
    uint256 public requiredConfirmations = 2;

    // Pending changes mapping: changeHash => executionTime
    mapping(bytes32 => uint256) public pendingChanges;
    
    // Change type enumeration for tracking
    enum ChangeType {
        MinProfit,
        MaxGasPrice,
        GasPriceTolerance,
        ExecutionCooldown,
        MaxSlippage,
        RequiredConfirmations
    }

    // Events
    event ParameterChangeQueued(
        bytes32 indexed changeHash,
        ChangeType indexed changeType,
        uint256 executionTime,
        bytes encodedData
    );
    
    event ParameterChangeExecuted(
        bytes32 indexed changeHash,
        ChangeType indexed changeType,
        uint256 executedAt
    );
    
    event ParameterChangeCancelled(
        bytes32 indexed changeHash,
        ChangeType indexed changeType
    );

    /**
     * @notice Constructor
     * @param _aavePool Aave V3 pool address
     * @param _uniswapV3Router Uniswap V3 router address
     * @param _sushiswapRouter Sushiswap router address
     */
    constructor(
        address _aavePool,
        address _uniswapV3Router,
        address _sushiswapRouter
    ) FlashArb(_aavePool, _uniswapV3Router, _sushiswapRouter) {}

    // ==================== TIMELOCK FUNCTIONS ====================

    /**
     * @notice Queue a parameter change with timelock
     * @param changeHash Unique hash of the change
     * @param changeType Type of parameter being changed
     * @param encodedData ABI-encoded change data
     */
    function queueParameterChange(
        bytes32 changeHash,
        ChangeType changeType,
        bytes memory encodedData
    ) public onlyOwner {
        require(pendingChanges[changeHash] == 0, "Change already queued");
        
        uint256 executionTime = block.timestamp + TIMELOCK_DELAY;
        pendingChanges[changeHash] = executionTime;
        
        emit ParameterChangeQueued(changeHash, changeType, executionTime, encodedData);
    }

    /**
     * @notice Cancel a pending parameter change
     * @param changeHash Hash of the change to cancel
     * @param changeType Type of parameter
     */
    function cancelParameterChange(
        bytes32 changeHash,
        ChangeType changeType
    ) external onlyOwner {
        require(pendingChanges[changeHash] != 0, "No pending change");
        
        delete pendingChanges[changeHash];
        
        emit ParameterChangeCancelled(changeHash, changeType);
    }

    /**
     * @dev Check if timelock has passed for a change
     * @param changeHash Hash of the change
     */
    modifier timelockPassed(bytes32 changeHash) {
        require(pendingChanges[changeHash] != 0, "Change not queued");
        require(
            block.timestamp >= pendingChanges[changeHash],
            "Timelock not expired"
        );
        _;
        // Clean up after execution
        delete pendingChanges[changeHash];
    }

    // ==================== TIMELOCKED PARAMETER CHANGES ====================

    /**
     * @notice Queue minimum profit change
     * @param _minProfitBps New minimum profit in basis points
     */
    function queueMinProfitChange(uint256 _minProfitBps) external onlyOwner {
        require(_minProfitBps > 0 && _minProfitBps <= 500, "Invalid profit BPS"); // Max 5%
        
        bytes32 changeHash = keccak256(
            abi.encode("setMinProfit", _minProfitBps, block.timestamp)
        );
        
        queueParameterChange(
            changeHash,
            ChangeType.MinProfit,
            abi.encode(_minProfitBps)
        );
    }

    /**
     * @notice Execute minimum profit change after timelock
     * @param _minProfitBps New minimum profit in basis points
     * @param queueTimestamp Timestamp when change was queued
     */
    function executeMinProfitChange(
        uint256 _minProfitBps,
        uint256 queueTimestamp
    ) external onlyOwner {
        bytes32 changeHash = keccak256(
            abi.encode("setMinProfit", _minProfitBps, queueTimestamp)
        );
        
        // Check timelock
        require(pendingChanges[changeHash] != 0, "Change not queued");
        require(
            block.timestamp >= pendingChanges[changeHash],
            "Timelock not expired"
        );
        
        // Execute change
        uint256 oldValue = minProfitBps;
        minProfitBps = _minProfitBps;
        
        // Clean up
        delete pendingChanges[changeHash];
        
        emit ParameterChangeExecuted(changeHash, ChangeType.MinProfit, block.timestamp);
        emit MinProfitUpdated(oldValue, _minProfitBps);
    }

    /**
     * @notice Queue max gas price change
     * @param newMaxGasPrice New maximum gas price
     */
    function queueMaxGasPriceChange(uint256 newMaxGasPrice) external onlyOwner {
        require(newMaxGasPrice >= 100 gwei, "Gas price too low");
        require(newMaxGasPrice <= 2000 gwei, "Gas price too high");
        
        bytes32 changeHash = keccak256(
            abi.encode("setMaxGasPrice", newMaxGasPrice, block.timestamp)
        );
        
        queueParameterChange(
            changeHash,
            ChangeType.MaxGasPrice,
            abi.encode(newMaxGasPrice)
        );
    }

    /**
     * @notice Execute max gas price change after timelock
     * @param newMaxGasPrice New maximum gas price
     * @param queueTimestamp Timestamp when change was queued
     */
    function executeMaxGasPriceChange(
        uint256 newMaxGasPrice,
        uint256 queueTimestamp
    ) external onlyOwner {
        bytes32 changeHash = keccak256(
            abi.encode("setMaxGasPrice", newMaxGasPrice, queueTimestamp)
        );
        
        require(pendingChanges[changeHash] != 0, "Change not queued");
        require(
            block.timestamp >= pendingChanges[changeHash],
            "Timelock not expired"
        );
        
        uint256 oldValue = maxGasPrice;
        maxGasPrice = newMaxGasPrice;
        
        delete pendingChanges[changeHash];
        
        emit ParameterChangeExecuted(changeHash, ChangeType.MaxGasPrice, block.timestamp);
        emit MaxGasPriceUpdated(oldValue, newMaxGasPrice);
    }

    /**
     * @notice Queue gas price tolerance change
     * @param newTolerance New gas price tolerance percentage
     */
    function queueGasPriceToleranceChange(uint256 newTolerance) external onlyOwner {
        require(newTolerance >= 100 && newTolerance <= 300, "Tolerance out of range");
        
        bytes32 changeHash = keccak256(
            abi.encode("setGasPriceTolerance", newTolerance, block.timestamp)
        );
        
        queueParameterChange(
            changeHash,
            ChangeType.GasPriceTolerance,
            abi.encode(newTolerance)
        );
    }

    /**
     * @notice Execute gas price tolerance change after timelock
     * @param newTolerance New tolerance value
     * @param queueTimestamp Timestamp when change was queued
     */
    function executeGasPriceToleranceChange(
        uint256 newTolerance,
        uint256 queueTimestamp
    ) external onlyOwner {
        bytes32 changeHash = keccak256(
            abi.encode("setGasPriceTolerance", newTolerance, queueTimestamp)
        );
        
        require(pendingChanges[changeHash] != 0, "Change not queued");
        require(
            block.timestamp >= pendingChanges[changeHash],
            "Timelock not expired"
        );
        
        uint256 oldValue = gasPriceTolerance;
        gasPriceTolerance = newTolerance;
        
        delete pendingChanges[changeHash];
        
        emit ParameterChangeExecuted(changeHash, ChangeType.GasPriceTolerance, block.timestamp);
        emit GasPriceToleranceUpdated(oldValue, newTolerance);
    }

    /**
     * @notice Queue execution cooldown change
     * @param newCooldown New cooldown period in seconds
     */
    function queueExecutionCooldownChange(uint256 newCooldown) external onlyOwner {
        require(newCooldown >= 10 && newCooldown <= 300, "Cooldown out of range");
        
        bytes32 changeHash = keccak256(
            abi.encode("setExecutionCooldown", newCooldown, block.timestamp)
        );
        
        queueParameterChange(
            changeHash,
            ChangeType.ExecutionCooldown,
            abi.encode(newCooldown)
        );
    }

    /**
     * @notice Execute execution cooldown change after timelock
     * @param newCooldown New cooldown value
     * @param queueTimestamp Timestamp when change was queued
     */
    function executeExecutionCooldownChange(
        uint256 newCooldown,
        uint256 queueTimestamp
    ) external onlyOwner {
        bytes32 changeHash = keccak256(
            abi.encode("setExecutionCooldown", newCooldown, queueTimestamp)
        );
        
        require(pendingChanges[changeHash] != 0, "Change not queued");
        require(
            block.timestamp >= pendingChanges[changeHash],
            "Timelock not expired"
        );
        
        uint256 oldValue = executionCooldown;
        executionCooldown = newCooldown;
        
        delete pendingChanges[changeHash];
        
        emit ParameterChangeExecuted(changeHash, ChangeType.ExecutionCooldown, block.timestamp);
        emit ExecutionCooldownUpdated(oldValue, newCooldown);
    }

    /**
     * @notice Queue max slippage change for a token
     * @param token Token address
     * @param maxSlippageBpsValue Maximum slippage in basis points
     */
    function queueMaxSlippageChange(
        address token,
        uint256 maxSlippageBpsValue
    ) external onlyOwner {
        require(token != address(0), "Invalid token");
        require(maxSlippageBpsValue <= MAX_SLIPPAGE_BPS, "Slippage too high");
        
        bytes32 changeHash = keccak256(
            abi.encode("setMaxSlippage", token, maxSlippageBpsValue, block.timestamp)
        );
        
        queueParameterChange(
            changeHash,
            ChangeType.MaxSlippage,
            abi.encode(token, maxSlippageBpsValue)
        );
    }

    /**
     * @notice Execute max slippage change after timelock
     * @param token Token address
     * @param maxSlippageBpsValue New slippage value
     * @param queueTimestamp Timestamp when change was queued
     */
    function executeMaxSlippageChange(
        address token,
        uint256 maxSlippageBpsValue,
        uint256 queueTimestamp
    ) external onlyOwner {
        bytes32 changeHash = keccak256(
            abi.encode("setMaxSlippage", token, maxSlippageBpsValue, queueTimestamp)
        );
        
        require(pendingChanges[changeHash] != 0, "Change not queued");
        require(
            block.timestamp >= pendingChanges[changeHash],
            "Timelock not expired"
        );
        
        uint256 oldValue = maxSlippageBps[token];
        maxSlippageBps[token] = maxSlippageBpsValue;
        
        delete pendingChanges[changeHash];
        
        emit ParameterChangeExecuted(changeHash, ChangeType.MaxSlippage, block.timestamp);
        emit MaxSlippageUpdated(token, oldValue, maxSlippageBpsValue);
    }

    /**
     * @notice Queue required confirmations change
     * @param _requiredConfirmations New required confirmations
     */
    function queueRequiredConfirmationsChange(
        uint256 _requiredConfirmations
    ) external onlyOwner {
        require(_requiredConfirmations >= 1 && _requiredConfirmations <= 10, "Invalid count");
        
        bytes32 changeHash = keccak256(
            abi.encode("setRequiredConfirmations", _requiredConfirmations, block.timestamp)
        );
        
        queueParameterChange(
            changeHash,
            ChangeType.RequiredConfirmations,
            abi.encode(_requiredConfirmations)
        );
    }

    /**
     * @notice Execute required confirmations change after timelock
     * @param _requiredConfirmations New value
     * @param queueTimestamp Timestamp when change was queued
     */
    function executeRequiredConfirmationsChange(
        uint256 _requiredConfirmations,
        uint256 queueTimestamp
    ) external onlyOwner {
        bytes32 changeHash = keccak256(
            abi.encode("setRequiredConfirmations", _requiredConfirmations, queueTimestamp)
        );
        
        require(pendingChanges[changeHash] != 0, "Change not queued");
        require(
            block.timestamp >= pendingChanges[changeHash],
            "Timelock not expired"
        );
        
        uint256 oldValue = requiredConfirmations;
        requiredConfirmations = _requiredConfirmations;
        
        delete pendingChanges[changeHash];
        
        emit ParameterChangeExecuted(changeHash, ChangeType.RequiredConfirmations, block.timestamp);
        emit RequiredConfirmationsUpdated(oldValue, _requiredConfirmations);
    }

    // ==================== EMERGENCY FUNCTIONS ====================

    /**
     * @notice Emergency pause (no timelock for safety)
     * @dev Can be called immediately in case of security issues
     */
    function emergencyPause() external onlyOwner {
        emergencyPaused = true;
        paused = true;
        emit EmergencyPauseTriggered(block.timestamp);
    }

    /**
     * @notice Check if a parameter change is pending
     * @param changeHash Hash of the change
     * @return isPending True if change is queued
     * @return executionTime Timestamp when change can be executed
     */
    function getPendingChange(bytes32 changeHash)
        external
        view
        returns (bool isPending, uint256 executionTime)
    {
        executionTime = pendingChanges[changeHash];
        isPending = executionTime != 0;
    }

    /**
     * @notice Get time remaining until change can be executed
     * @param changeHash Hash of the change
     * @return timeRemaining Seconds remaining (0 if ready or not queued)
     */
    function getTimelockRemaining(bytes32 changeHash)
        external
        view
        returns (uint256 timeRemaining)
    {
        uint256 executionTime = pendingChanges[changeHash];
        if (executionTime == 0 || block.timestamp >= executionTime) {
            return 0;
        }
        return executionTime - block.timestamp;
    }

    // Additional events for timelock operations
    event MinProfitUpdated(uint256 indexed oldValue, uint256 indexed newValue);
    event MaxGasPriceUpdated(uint256 indexed oldValue, uint256 indexed newValue);
    event GasPriceToleranceUpdated(uint256 indexed oldValue, uint256 indexed newValue);
    event ExecutionCooldownUpdated(uint256 indexed oldValue, uint256 indexed newValue);
    event MaxSlippageUpdated(address indexed token, uint256 oldValue, uint256 newValue);
    event RequiredConfirmationsUpdated(uint256 indexed oldValue, uint256 indexed newValue);
    event EmergencyPauseTriggered(uint256 indexed timestamp);
}

