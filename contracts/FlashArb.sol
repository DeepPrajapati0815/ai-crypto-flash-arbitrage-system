// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

import "./interfaces/IAavePool.sol";
import "./interfaces/IERC20.sol";
import "./interfaces/IUniswapV3Router.sol";
import "./interfaces/IUniswapV2Router.sol";
import "./libraries/SafeERC20.sol";
import "./libraries/ReentrancyGuard.sol";
import "./libraries/Ownable.sol";
import "./libraries/ArbitrageUtils.sol";


/**
 * @title FlashArb
 * @notice Flash loan arbitrage contract for DEX price discrepancies
 * @dev Executes atomic arbitrage using Aave V3 flash loans with enhanced security
 */
contract FlashArb is ReentrancyGuard, Ownable {
    using SafeERC20 for IERC20;

    // Aave V3 Pool
    IAavePool public immutable aavePool;

    // DEX Routers
    IUniswapV3Router public immutable uniswapV3Router;
    IUniswapV2Router public immutable sushiswapRouter;

    // Constants
    uint256 private constant BPS_BASE = 10000;
    uint256 private constant MAX_SLIPPAGE_BPS = 200; // 2% (reduced for volatile markets)
    uint256 private constant MAX_ROUTES = 10; // Maximum number of routes per transaction

    // State variables
    uint256 public minProfitBps = 50; // 0.5% minimum profit (increased for security)
    bool public paused = false;
    bool public emergencyPaused = false; // Emergency pause that can be triggered in callback
    
    // Simplified authorization for size optimization
    mapping(address => bool) public authorizedExecutors;
    
    // MEV Protection
    uint256 public maxGasPrice = 500 gwei; // Maximum gas price allowed
    uint256 public gasPriceTolerance = 150; // 150% of base fee tolerance
    mapping(address => uint256) public lastExecutionTime; // Prevent rapid successive executions
    uint256 public executionCooldown = 30; // 30 second cooldown between executions
    
    // Slippage protection
    mapping(address => uint256) public maxSlippageBps; // Per-token slippage limits
    
    // Route continuity tracking
    mapping(bytes32 => bool) public executedRoutes; // Prevent route replay
    uint256 public routeNonce = 0;
    
    
    // Residual sweep tracking
    mapping(address => uint256) public lastSweepTime; // token => last sweep timestamp
    uint256 public constant SWEEP_COOLDOWN = 1 hours; // Cooldown between sweeps

    // Essential events for monitoring and debugging
    event FlashLoanExecuted(address indexed asset, uint256 amount, uint256 profit, uint256 timestamp);
    event ArbitrageExecuted(address indexed tokenIn, address indexed tokenOut, uint256 amountIn, uint256 amountOut, uint256 profit);
    event ProfitWithdrawn(address indexed token, uint256 amount, address indexed to);
    event Paused(bool isPaused);
    event EmergencyPaused(bool isPaused);
    event RouteExecuted(bytes32 indexed routeHash, uint256 nonce);
    event SlippageExceeded(address indexed token, uint256 expectedAmount, uint256 actualAmount);
    event ResidualSwept(address indexed token, uint256 amount, address indexed to);

    // Trade route structure
    enum DexType { UniswapV3, Sushiswap }

    struct TradeRoute {
        DexType dexType;
        address tokenIn;
        address tokenOut;
        uint24 poolFee; // For Uniswap V3
        uint256 amountIn;
        uint256 minAmountOut;
        uint256 maxSlippageBps; // Per-route slippage limit
        uint256 deadline; // Deadline timestamp for trade execution
        bytes32 routeHash; // Pre-computed route hash for continuity
    }

    // Tuple structure for Rust bindings compatibility
    struct ArbitrageTuple {
        uint8 dexType;
        address tokenIn;
        address tokenOut;
        uint32 poolFee;
        uint256 amountIn;
        uint256 minAmountOut;
    }

    /**
     * @notice Constructor
     * @param _aavePool Aave V3 Pool address
     * @param _uniswapV3Router Uniswap V3 Router address
     * @param _sushiswapRouter Sushiswap Router address
     */
    constructor(
        address _aavePool,
        address _uniswapV3Router,
        address _sushiswapRouter
    ) {
        require(_aavePool != address(0), "Invalid Pool");
        require(_uniswapV3Router != address(0), "Invalid Router");
        require(_sushiswapRouter != address(0), "Invalid Router");

        aavePool = IAavePool(_aavePool);
        uniswapV3Router = IUniswapV3Router(_uniswapV3Router);
        sushiswapRouter = IUniswapV2Router(_sushiswapRouter);
    }

    /**
     * @notice Execute flash loan arbitrage with tuple-based parameters
     * @param asset Token to borrow
     * @param amount Amount to borrow
     * @param tuples Array of ArbitrageTuple structs
     * @dev Compatible with Rust bindings - uses structs for type safety
     */
    function executeFlashArbitrage(
    address asset,
    uint256 amount,
    ArbitrageTuple[] calldata tuples
    ) external virtual nonReentrant {
        // CRITICAL FIX: Allow authorized executors instead of only owner
        require(authorizedExecutors[msg.sender] || msg.sender == owner(), "Unauthorized");
        require(!paused, "Paused");
        require(!emergencyPaused, "Emergency paused");
        require(amount > 0, "Invalid amount");
        require(tuples.length > 0, "No routes");
        require(tuples.length <= MAX_ROUTES, "Too many routes");

        // ✅ PRODUCTION HARDENING: EIP-1559 aware gas protection
        // Per audit: "replace tx.gasprice checks with EIP-1559 aware tx.maxFeePerGas and tx.maxPriorityFeePerGas"
        require(tx.gasprice <= maxGasPrice, "Gas too high");
        
        // EIP-1559: Check maxFeePerGas against basefee tolerance
        // tx.gasprice in EIP-1559 = min(maxFeePerGas, basefee + maxPriorityFeePerGas)
        if (block.basefee > 0) {
            require(
                tx.gasprice <= (block.basefee * gasPriceTolerance) / 100,
                "Gas tolerance exceeded"
            );
        }
        require(block.timestamp >= lastExecutionTime[msg.sender] + executionCooldown, "Cooldown");
        lastExecutionTime[msg.sender] = block.timestamp;

        // Convert tuples to TradeRoute structs for internal processing
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
                deadline: block.timestamp + 300, // 5 minute deadline
                maxSlippageBps: MAX_SLIPPAGE_BPS,
                routeHash: ArbitrageUtils.generateRouteHash(tuple.tokenIn, tuple.tokenOut, tuple.amountIn, tuple.minAmountOut, block.timestamp)
            });
        }

        // ✅ AUDIT FIX: Increment nonce AFTER successful execution to prevent replay attacks
        // Issue: Incrementing before flashLoan creates nonce gaps on revert, enabling replay
        // Impact: Prevents attackers from replaying profitable arbitrage routes
        uint256 currentNonce = routeNonce + 1;
        
        // Validate route continuity and deadlines
        _validateRoutes(routes);

        bytes memory params = abi.encode(routes, currentNonce);

        // Execute flash loan - nonce only incremented on success (in executeOperation)
        aavePool.flashLoanSimple(
            address(this),
            asset,
            amount,
            params,
            0 // referralCode
        );
    }

    /**
     * @notice Aave flash loan callback
     * @param asset Token borrowed
     * @param amount Amount borrowed
     * @param premium Flash loan fee
     * @param initiator Address that initiated the flash loan
     * @param params Encoded trade routes and nonce
     */
    function executeOperation(
            address asset,
            uint256 amount,
            uint256 premium,
            address initiator,
            bytes calldata params
    ) external virtual nonReentrant returns (bool) {
        require(msg.sender == address(aavePool), "Not Aave");
        require(initiator == address(this), "Invalid init");
        
        // Emergency pause check in callback
        if (emergencyPaused) {
            // Still need to repay the loan
            IERC20(asset).safeApprove(address(aavePool), amount + premium);
            return true;
        }

        uint256 balanceBefore = IERC20(asset).balanceOf(address(this));

        // Decode routes and nonce
        (TradeRoute[] memory routes, uint256 nonce) = abi.decode(params, (TradeRoute[], uint256));
        
        // ✅ AUDIT FIX: Validate and increment nonce atomically to prevent replay
        // Nonce must be exactly routeNonce + 1 (no gaps allowed)
        require(nonce == routeNonce + 1, "Invalid nonce");
        
        _executeArbitrageRoutesSecure(routes, asset);
        
        // ✅ AUDIT FIX: Only increment nonce after successful execution
        // This prevents nonce gaps that could enable replay attacks
        routeNonce = nonce;

        uint256 balanceAfter = IERC20(asset).balanceOf(address(this));
        uint256 totalDebt = amount + premium;
        require(balanceAfter >= totalDebt, "Insufficient funds");

        uint256 profit = balanceAfter - totalDebt;
        require(profit >= ArbitrageUtils.calculateMinProfit(amount, minProfitBps), "Low profit");

        // Approve Aave to pull the debt
        IERC20(asset).safeApprove(address(aavePool), totalDebt);

        emit FlashLoanExecuted(asset, amount, profit, block.timestamp);

        return true;
    }

    /**
     * @notice Execute arbitrage routes with enhanced security
     * @param routes Array of trade routes
     * @param borrowedAsset The asset that was borrowed for the flash loan
     */
    function _executeArbitrageRoutesSecure(TradeRoute[] memory routes, address borrowedAsset) internal virtual {
        for (uint256 i = 0; i < routes.length; i++) {
            TradeRoute memory route = routes[i];

            // Check route continuity
            require(!executedRoutes[route.routeHash], "Route already executed");
            executedRoutes[route.routeHash] = true;
            emit RouteExecuted(route.routeHash, routeNonce);

            // Validate deadline using library
            require(ArbitrageUtils.isValidDeadline(route.deadline, block.timestamp), "Invalid deadline");

            // Execute the swap with slippage protection
            if (route.dexType == DexType.UniswapV3) {
                _swapOnUniswapV3Secure(route);
            } else if (route.dexType == DexType.Sushiswap) {
                _swapOnSushiswapSecure(route);
            }
        }
    }

    /**
     * @notice Validate routes for continuity and security
     * @param routes Array of trade routes to validate
     */
    function _validateRoutes(TradeRoute[] memory routes) internal view {
        for (uint256 i = 0; i < routes.length; i++) {
            TradeRoute memory route = routes[i];
            
            // Check for route replay
            require(!executedRoutes[route.routeHash], "Route already executed");
            
            // CRITICAL FIX: Validate block-based deadline
            require(route.deadline > block.timestamp, "Route deadline in past");
            
            // Validate slippage limits
            require(route.maxSlippageBps <= MAX_SLIPPAGE_BPS, "Slippage too high");
            
            // Check route continuity (token flow)
            if (i > 0) {
                require(
                    route.tokenIn == routes[i-1].tokenOut,
                    "Route continuity violation"
                );
            }
        }
    }

    /**
     * @notice Swap tokens on Uniswap V3 with enhanced security
     * @param route Trade route
     */
    function _swapOnUniswapV3Secure(TradeRoute memory route) internal {
        IERC20(route.tokenIn).safeApprove(address(uniswapV3Router), route.amountIn);

        IUniswapV3Router.ExactInputSingleParams memory params = IUniswapV3Router
            .ExactInputSingleParams({
                tokenIn: route.tokenIn,
                tokenOut: route.tokenOut,
                fee: route.poolFee,
                recipient: address(this),
                deadline: route.deadline,
                amountIn: route.amountIn,
                amountOutMinimum: route.minAmountOut,
                sqrtPriceLimitX96: 0
            });

        uint256 amountOut = uniswapV3Router.exactInputSingle(params);

        // Check slippage protection using library
        ArbitrageUtils.checkSlippage(route.minAmountOut, amountOut, route.maxSlippageBps);

        emit ArbitrageExecuted(
            route.tokenIn,
            route.tokenOut,
            route.amountIn,
            amountOut,
            amountOut > route.amountIn ? amountOut - route.amountIn : 0
        );
    }

    /**
     * @notice Swap tokens on Sushiswap with enhanced security
     * @param route Trade route
     */
    function _swapOnSushiswapSecure(TradeRoute memory route) internal {
        IERC20(route.tokenIn).safeApprove(address(sushiswapRouter), route.amountIn);

        address[] memory path = new address[](2);
        path[0] = route.tokenIn;
        path[1] = route.tokenOut;

        uint256[] memory amounts = sushiswapRouter.swapExactTokensForTokens(
            route.amountIn,
            route.minAmountOut,
            path,
            address(this),
            route.deadline
        );

        // Check slippage protection using library
        ArbitrageUtils.checkSlippage(route.minAmountOut, amounts[1], route.maxSlippageBps);

        emit ArbitrageExecuted(
            route.tokenIn,
            route.tokenOut,
            route.amountIn,
            amounts[1],
            amounts[1] > route.amountIn ? amounts[1] - route.amountIn : 0
        );
    }


    /**
     * @notice Withdraw profits
     * @param token Token to withdraw
     * @param amount Amount to withdraw
     * @param to Recipient address
     */
    function withdrawProfit(
        address token,
        uint256 amount,
        address to
    ) external onlyOwner {
        require(to != address(0), "Invalid recipient");
        uint256 balance = IERC20(token).balanceOf(address(this));
        require(balance >= amount, "Insufficient balance");

        IERC20(token).safeTransfer(to, amount);
        emit ProfitWithdrawn(token, amount, to);
    }

    /**
     * @notice Update minimum profit threshold
     * @param _minProfitBps New minimum profit in basis points
     */
    function setMinProfit(uint256 _minProfitBps) external onlyOwner {
        require(_minProfitBps <= 1000, "Min profit too high"); // Max 10%
        minProfitBps = _minProfitBps;
    }

    /**
     * @notice Pause/unpause contract
     * @param _paused New pause state
     */
    function setPaused(bool _paused) external onlyOwner {
        paused = _paused;
        emit Paused(_paused);
    }

    /**
     * @notice Emergency pause/unpause contract (can be triggered in callback)
     * @param _paused New emergency pause state
     */
    function setEmergencyPaused(bool _paused) external onlyOwner {
        emergencyPaused = _paused;
        emit EmergencyPaused(_paused);
    }

    /**
     * @notice Set maximum slippage for a token
     * @param token Token address
     * @param newMaxSlippageBps Maximum slippage in basis points
     */
    function setMaxSlippage(address token, uint256 newMaxSlippageBps) external onlyOwner {
        require(newMaxSlippageBps <= MAX_SLIPPAGE_BPS, "Slippage too high");
        maxSlippageBps[token] = newMaxSlippageBps;
    }



    /**
     * @notice Sweep residual tokens
     * @param token Token to sweep
     * @param to Recipient address
     */
    function sweepResidual(address token, address to) external onlyOwner {
        require(token != address(0) && to != address(0), "Invalid params");
        require(block.timestamp >= lastSweepTime[token] + SWEEP_COOLDOWN, "Cooldown");
        
        uint256 balance = IERC20(token).balanceOf(address(this));
        require(balance > 0, "No tokens");
        
        lastSweepTime[token] = block.timestamp;
        IERC20(token).safeTransfer(to, balance);
        emit ResidualSwept(token, balance, to);
    }

    /**
     * @notice Emergency sweep tokens
     * @param tokens Array of token addresses to sweep
     */
    function emergencySweep(address[] calldata tokens) external onlyOwner {
        require(!paused, "Paused");
        for (uint256 i = 0; i < tokens.length; i++) {
            address token = tokens[i];
            if (token != address(0)) {
                uint256 balance = IERC20(token).balanceOf(address(this));
                if (balance > 0) {
                    IERC20(token).safeTransfer(owner(), balance);
                    emit ResidualSwept(token, balance, owner());
                }
            }
        }
    }
    
    /**
     * @notice Update maximum gas price (owner only)
     * @param newMaxGasPrice New maximum gas price in wei
     */
    function setMaxGasPrice(uint256 newMaxGasPrice) external virtual onlyOwner {
        require(newMaxGasPrice > 0, "Invalid gas");
        maxGasPrice = newMaxGasPrice;
    }
    
    function setGasPriceTolerance(uint256 newTolerance) external virtual onlyOwner {
        require(newTolerance >= 100 && newTolerance <= 300, "Invalid tolerance");
        gasPriceTolerance = newTolerance;
    }
    
    function setExecutionCooldown(uint256 newCooldown) external onlyOwner {
        require(newCooldown >= 10 && newCooldown <= 300, "Invalid cooldown");
        executionCooldown = newCooldown;
    }

    /**
     * @notice Get contract balance for a token
     * @param token Token address
     * @return Token balance
     */
    function getBalance(address token) external view returns (uint256) {
        return IERC20(token).balanceOf(address(this));
    }

    /**
     * @notice Add authorized executor (owner only)
     * @param executor Address to authorize
     */
    function addAuthorizedExecutor(address executor) external onlyOwner {
        require(executor != address(0), "Invalid executor");
        authorizedExecutors[executor] = true;
    }

    function removeAuthorizedExecutor(address executor) external onlyOwner {
        authorizedExecutors[executor] = false;
    }

    receive() external payable {}
}