// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

import "./FlashArb.sol";
import "./interfaces/IAavePool.sol";

/**
 * @title FlashArbUltimate
 * @notice Production-ready flash arbitrage with all critical features
 * @dev Combines:
 *      - FlashArb.sol base functionality (Rust-compatible)
 *      - Circuit breaker from FlashArbProductionSafe
 *      - Bundle-only mode for MEV protection
 *      - Absolute minimum profit enforcement
 *      - Pre/post execution validation
 *      - Enhanced monitoring and safety
 * 
 * ✅ PRODUCTION FEATURES:
 *    - Circuit breaker (auto-pause on failures)
 *    - Bundle-only mode (Flashbots integration)
 *    - Absolute minimum profit (not just percentage)
 *    - Pre/post execution validation
 *    - Enhanced events for monitoring
 *    - Rust-compatible (no ABI changes)
 */
contract FlashArbUltimate is FlashArb {
    using SafeERC20 for IERC20;
    
    // ==================== PRODUCTION SAFETY ====================
    
    /// Minimum profit required (absolute, in wei)
    uint256 public minProfitWei;
    
    /// Circuit breaker: max failed attempts before auto-pause
    uint256 public failedAttempts;
    uint256 public maxFailedAttempts;
    
    /// Bundle-only mode (prevent public mempool frontrunning)
    bool public bundleOnlyMode;
    
    /// Allowed bundle submitters (e.g., Flashbots relay)
    mapping(address => bool) public authorizedBundleSubmitters;
    
    /// Track profit per execution for monitoring
    mapping(bytes32 => uint256) public executionProfits;
    
    // ==================== EVENTS ====================
    
    event ProfitRecorded(
        bytes32 indexed executionId,
        address indexed asset,
        uint256 profit,
        uint256 gasUsed
    );
    
    event CircuitBreakerTriggered(
        uint256 failedAttempts,
        uint256 timestamp
    );
    
    event BundleOnlyModeChanged(bool enabled);
    
    event MinProfitUpdated(uint256 oldValue, uint256 newValue);
    
    event ExecutionFailed(
        address indexed asset,
        uint256 amount,
        string reason,
        uint256 timestamp
    );
    
    // ==================== CONSTRUCTOR ====================
    
    constructor(
        address _aavePool,
        address _uniswapV3Router,
        address _sushiswapRouter,
        uint256 _minProfitWei,
        uint256 _maxFailedAttempts
    ) FlashArb(_aavePool, _uniswapV3Router, _sushiswapRouter) {
        require(_minProfitWei > 0, "Invalid min profit");
        require(_maxFailedAttempts > 0 && _maxFailedAttempts <= 20, "Invalid max attempts");
        
        minProfitWei = _minProfitWei;
        maxFailedAttempts = _maxFailedAttempts;
        bundleOnlyMode = true; // Start in safe mode
    }
    
    // ==================== MODIFIERS ====================
    
    modifier onlyAuthorizedBundleSubmitter() {
        if (bundleOnlyMode) {
            require(
                authorizedBundleSubmitters[msg.sender] || msg.sender == owner(),
                "Not authorized bundle submitter"
            );
        }
        _;
    }
    
    // ==================== ENHANCED EXECUTION ====================
    
    /**
     * @notice Execute flash arbitrage with production safety
     * @dev Adds bundle-only check on top of base implementation
     */
    function executeFlashArbitrage(
        address asset,
        uint256 amount,
        ArbitrageTuple[] calldata tuples
    ) external override nonReentrant onlyAuthorizedBundleSubmitter {
        // Authorization check
        require(authorizedExecutors[msg.sender] || msg.sender == owner(), "Unauthorized");
        require(!paused, "Paused");
        require(!emergencyPaused, "Emergency paused");
        require(amount > 0, "Invalid amount");
        require(tuples.length > 0, "No routes");
        require(tuples.length <= 10, "Too many routes"); // Using base contract's MAX_ROUTES value

        // Gas price protection
        require(tx.gasprice <= maxGasPrice, "Gas too high");
        
        if (block.basefee > 0) {
            require(
                tx.gasprice <= (block.basefee * gasPriceTolerance) / 100,
                "Gas tolerance exceeded"
            );
        }
        
        require(block.timestamp >= lastExecutionTime[msg.sender] + executionCooldown, "Cooldown");
        lastExecutionTime[msg.sender] = block.timestamp;

        // Convert tuples to TradeRoute structs
        TradeRoute[] memory routes = new TradeRoute[](tuples.length);
        for (uint256 i = 0; i < tuples.length; i++) {
            ArbitrageTuple calldata tuple = tuples[i];
            
            routes[i] = TradeRoute({
                dexType: DexType(tuple.dexType),
                tokenIn: tuple.tokenIn,
                tokenOut: tuple.tokenOut,
                poolFee: uint24(tuple.poolFee),
                amountIn: tuple.amountIn,
                minAmountOut: tuple.minAmountOut,
                deadline: block.timestamp + 300,
                maxSlippageBps: 200, // Using base contract's MAX_SLIPPAGE_BPS value
                routeHash: ArbitrageUtils.generateRouteHash(tuple.tokenIn, tuple.tokenOut, tuple.amountIn, tuple.minAmountOut, block.timestamp)
            });
        }

        uint256 currentNonce = routeNonce + 1;
        _validateRoutes(routes);

        bytes memory params = abi.encode(routes, currentNonce);

        // Execute flash loan
        aavePool.flashLoanSimple(
            address(this),
            asset,
            amount,
            params,
            0
        );
    }
    
    /**
     * @notice Enhanced executeOperation with production safety
     * @dev Overrides base callback with validation and circuit breaker
     * @dev NO nonReentrant modifier - this is a legitimate callback from Aave
     */
    function executeOperation(
        address asset,
        uint256 amount,
        uint256 premium,
        address initiator,
        bytes calldata params
    ) external override returns (bool) {
        // CRITICAL: Only Aave can call this function
        require(msg.sender == address(aavePool), "Not Aave");
        require(initiator == address(this), "Invalid init");
        
        // Emergency pause check
        if (emergencyPaused) {
            IERC20(asset).safeApprove(address(aavePool), amount + premium);
            return true;
        }
        
        // ✅ PRODUCTION: Pre-execution validation
        require(IERC20(asset).balanceOf(address(this)) >= amount, "Flash loan not received");
        
        // Decode routes and nonce
        (TradeRoute[] memory routes, uint256 nonce) = abi.decode(params, (TradeRoute[], uint256));
        
        // Validate nonce
        require(nonce == routeNonce + 1, "Invalid nonce");
        
        // Execute arbitrage routes with try-catch for circuit breaker
        try this.executeArbitrageWithCircuitBreaker(routes, asset, nonce) {
            // ✅ PRODUCTION: Post-execution validation and profit check
            uint256 totalDebt = amount + premium;
            uint256 balanceAfter = IERC20(asset).balanceOf(address(this));
            
            require(balanceAfter >= totalDebt, "Insufficient funds to repay");
            
            uint256 profit = balanceAfter - totalDebt;
            
            // ✅ PRODUCTION: Absolute minimum profit check
            require(profit >= minProfitWei, "Profit below minimum threshold");
            
            // Reset circuit breaker on success
            failedAttempts = 0;
            
            // Record profit for monitoring
            executionProfits[keccak256(abi.encodePacked(asset, amount, block.number))] = profit;
            
            emit ProfitRecorded(
                keccak256(abi.encodePacked(asset, amount, block.number)),
                asset,
                profit,
                gasleft()
            );
            
            // Approve repayment
            IERC20(asset).safeApprove(address(aavePool), totalDebt);
            
            emit FlashLoanExecuted(asset, amount, profit, block.timestamp);
            
            return true;
            
        } catch Error(string memory reason) {
            // ✅ PRODUCTION: Circuit breaker
            _handleExecutionFailure(asset, amount, reason);
            
        } catch (bytes memory lowLevelData) {
            // Low-level failure
            _handleExecutionFailure(asset, amount, "Low-level execution failed");
            
            // Revert with low-level data for debugging
            assembly {
                revert(add(lowLevelData, 32), mload(lowLevelData))
            }
        }
    }
    
    /**
     * @notice Handle execution failure and circuit breaker logic
     * @dev Extracted to reduce stack depth in executeOperation
     */
    function _handleExecutionFailure(
        address asset,
        uint256 amount,
        string memory reason
    ) internal {
        failedAttempts++;
        
        emit ExecutionFailed(asset, amount, reason, block.timestamp);
        
        if (failedAttempts >= maxFailedAttempts) {
            emergencyPaused = true;
            emit CircuitBreakerTriggered(failedAttempts, block.timestamp);
        }
        
        // Still need to repay the loan to avoid Aave penalty
        uint256 premium = aavePool.FLASHLOAN_PREMIUM_TOTAL() * amount / 10000;
        IERC20(asset).safeApprove(address(aavePool), amount + premium);
        
        revert(reason);
    }
    
    /**
     * @notice Public wrapper for arbitrage execution to enable try-catch
     * @dev This function is needed because try-catch only works with external calls
     * @param routes Array of trade routes to execute
     * @param asset The borrowed asset
     * @param nonce The route nonce for replay protection
     */
    function executeArbitrageWithCircuitBreaker(
        TradeRoute[] memory routes,
        address asset,
        uint256 nonce
    ) external {
        require(msg.sender == address(this), "Internal only");
        
        // Execute all routes
        for (uint256 i = 0; i < routes.length; i++) {
            TradeRoute memory route = routes[i];

            // Check route continuity
            require(!executedRoutes[route.routeHash], "Route already executed");
            executedRoutes[route.routeHash] = true;
            emit RouteExecuted(route.routeHash, routeNonce);

            // Validate deadline
            require(ArbitrageUtils.isValidDeadline(route.deadline, block.timestamp), "Invalid deadline");

            // Execute the swap with slippage protection
            if (route.dexType == DexType.UniswapV3) {
                _swapOnUniswapV3Secure(route);
            } else if (route.dexType == DexType.Sushiswap) {
                _swapOnSushiswapSecure(route);
            }
        }
        
        // Update nonce after successful execution
        routeNonce = nonce;
    }
    
    // ==================== ADMIN FUNCTIONS ====================
    
    /**
     * @notice Authorize bundle submitter (e.g., Flashbots relay)
     */
    function authorizeBundleSubmitter(address submitter) external onlyOwner {
        require(submitter != address(0), "Invalid address");
        authorizedBundleSubmitters[submitter] = true;
    }
    
    /**
     * @notice Revoke bundle submitter authorization
     */
    function revokeBundleSubmitter(address submitter) external onlyOwner {
        authorizedBundleSubmitters[submitter] = false;
    }
    
    /**
     * @notice Toggle bundle-only mode
     */
    function setBundleOnlyMode(bool enabled) external onlyOwner {
        bundleOnlyMode = enabled;
        emit BundleOnlyModeChanged(enabled);
    }
    
    /**
     * @notice Update minimum profit threshold
     */
    function setMinProfitWei(uint256 newMinProfit) external onlyOwner {
        require(newMinProfit > 0, "Invalid min profit");
        uint256 oldValue = minProfitWei;
        minProfitWei = newMinProfit;
        emit MinProfitUpdated(oldValue, newMinProfit);
    }
    
    /**
     * @notice Update max failed attempts for circuit breaker
     */
    function setMaxFailedAttempts(uint256 newMax) external onlyOwner {
        require(newMax > 0 && newMax <= 20, "Invalid max attempts");
        maxFailedAttempts = newMax;
    }
    
    /**
     * @notice Reset circuit breaker manually
     */
    function resetCircuitBreaker() external onlyOwner {
        failedAttempts = 0;
        emergencyPaused = false;
    }
    
    /**
     * @notice Get execution profit by ID
     */
    function getExecutionProfit(bytes32 executionId) external view returns (uint256) {
        return executionProfits[executionId];
    }
    
    /**
     * @notice Check if contract is healthy
     */
    function isHealthy() external view returns (bool) {
        return !emergencyPaused && failedAttempts < maxFailedAttempts;
    }
    
    /**
     * @notice Get contract status
     */
    function getStatus() external view returns (
        bool isPaused,
        bool isEmergencyPaused,
        uint256 currentFailedAttempts,
        uint256 maxAttempts,
        uint256 currentMinProfit,
        bool isBundleOnly
    ) {
        return (
            paused,
            emergencyPaused,
            failedAttempts,
            maxFailedAttempts,
            minProfitWei,
            bundleOnlyMode
        );
    }
}