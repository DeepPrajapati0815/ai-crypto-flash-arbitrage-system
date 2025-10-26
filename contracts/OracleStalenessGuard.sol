// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

/**
 * @title OracleStalenessGuard
 * @notice ✅ AUDIT FIX ISSUE #7/11: Oracle staleness protection for price feeds
 * @dev Prevents execution with stale oracle data that could lead to losses
 * 
 * This addresses the audit finding that oracle timestamps were not validated,
 * creating risk of using outdated prices for arbitrage execution.
 */
abstract contract OracleStalenessGuard {
    /// Maximum allowed age for oracle data (60 seconds default)
    uint256 public maxOracleStalenessPeriod = 60;
    
    /// Heartbeat interval for different oracle types (in seconds)
    mapping(address => uint256) public oracleHeartbeats;
    
    /// Events
    event OracleStalenessPeriodUpdated(uint256 oldValue, uint256 newValue);
    event OracleHeartbeatConfigured(address indexed oracle, uint256 heartbeat);
    event StaleOracleDetected(address indexed oracle, uint256 oracleTimestamp, uint256 currentTimestamp, uint256 age);
    
    /**
     * @notice Configure heartbeat for a specific oracle
     * @param oracle Oracle contract address
     * @param heartbeat Expected update frequency in seconds
     * @dev Common values:
     *      - Chainlink ETH/USD: 3600 (1 hour)
     *      - Uniswap TWAP: 1800 (30 min)
     *      - High-frequency: 60 (1 min)
     */
    function _setOracleHeartbeat(address oracle, uint256 heartbeat) internal {
        require(oracle != address(0), "Invalid oracle address");
        require(heartbeat > 0 && heartbeat <= 86400, "Heartbeat must be between 1s and 24h");
        
        oracleHeartbeats[oracle] = heartbeat;
        emit OracleHeartbeatConfigured(oracle, heartbeat);
    }
    
    /**
     * @notice Update maximum staleness period
     * @param newPeriod New maximum age in seconds
     */
    function _setMaxOracleStalenessPeriod(uint256 newPeriod) internal {
        require(newPeriod >= 10 && newPeriod <= 3600, "Period must be between 10s and 1h");
        
        uint256 oldValue = maxOracleStalenessPeriod;
        maxOracleStalenessPeriod = newPeriod;
        
        emit OracleStalenessPeriodUpdated(oldValue, newPeriod);
    }
    
    /**
     * ✅ AUDIT FIX ISSUE #MP2: Configure common oracle heartbeats for production
     * @notice Batch configure heartbeats for common Chainlink price feeds
     * @param oracles Array of oracle addresses
     * @param heartbeats Array of heartbeat values (in seconds)
     * @dev Common Chainlink heartbeat values:
     *      - ETH/USD (mainnet): 3600 (1 hour)
     *      - BTC/USD (mainnet): 3600 (1 hour)
     *      - USDC/USD (mainnet): 86400 (24 hours)
     *      - DAI/USD (mainnet): 3600 (1 hour)
     *      - High-volatility pairs: 600 (10 minutes)
     * 
     *      Arbitrum/Optimism typically have shorter heartbeats (300-600 seconds)
     */
    function _configureCommonOracleHeartbeats(
        address[] memory oracles, 
        uint256[] memory heartbeats
    ) internal {
        require(oracles.length == heartbeats.length, "Array length mismatch");
        require(oracles.length > 0, "Empty arrays");
        
        for (uint256 i = 0; i < oracles.length; i++) {
            _setOracleHeartbeat(oracles[i], heartbeats[i]);
        }
    }
    
    /**
     * ✅ AUDIT FIX ISSUE #MP2: Helper to set default heartbeats for Ethereum Mainnet
     * @notice Configure standard heartbeats for common Chainlink feeds on Ethereum mainnet
     * @param ethUsdOracle ETH/USD price feed address
     * @param btcUsdOracle BTC/USD price feed address
     * @param usdcUsdOracle USDC/USD price feed address
     * @param daiUsdOracle DAI/USD price feed address
     */
    function _configureMainnetOracleHeartbeats(
        address ethUsdOracle,
        address btcUsdOracle,
        address usdcUsdOracle,
        address daiUsdOracle
    ) internal {
        if (ethUsdOracle != address(0)) {
            _setOracleHeartbeat(ethUsdOracle, 3600);  // 1 hour
        }
        if (btcUsdOracle != address(0)) {
            _setOracleHeartbeat(btcUsdOracle, 3600);  // 1 hour
        }
        if (usdcUsdOracle != address(0)) {
            _setOracleHeartbeat(usdcUsdOracle, 86400);  // 24 hours (stable)
        }
        if (daiUsdOracle != address(0)) {
            _setOracleHeartbeat(daiUsdOracle, 3600);  // 1 hour
        }
    }
    
    /**
     * ✅ AUDIT FIX ISSUE #MP2: Helper for L2 chains (shorter heartbeats)
     * @notice Configure heartbeats for Arbitrum/Optimism (typically more frequent updates)
     * @param oracles Array of L2 oracle addresses
     */
    function _configureL2OracleHeartbeats(address[] memory oracles) internal {
        uint256 l2Heartbeat = 600; // 10 minutes (typical for L2s)
        
        for (uint256 i = 0; i < oracles.length; i++) {
            if (oracles[i] != address(0)) {
                _setOracleHeartbeat(oracles[i], l2Heartbeat);
            }
        }
    }
    
    /**
     * @notice Check if oracle data is fresh
     * @param oracleTimestamp Timestamp from oracle data
     * @return isFresh True if data is within staleness period
     * @return age Age of oracle data in seconds
     */
    function _checkOracleFreshness(uint256 oracleTimestamp) 
        internal 
        view 
        returns (bool isFresh, uint256 age) 
    {
        require(oracleTimestamp > 0, "Invalid oracle timestamp");
        require(oracleTimestamp <= block.timestamp, "Oracle timestamp in future");
        
        age = block.timestamp - oracleTimestamp;
        isFresh = age < maxOracleStalenessPeriod;
        
        return (isFresh, age);
    }
    
    /**
     * @notice Check oracle freshness and revert if stale
     * @param oracle Oracle address (for logging)
     * @param oracleTimestamp Timestamp from oracle
     */
    function _requireFreshOracle(address oracle, uint256 oracleTimestamp) internal view {
        (bool isFresh, uint256 age) = _checkOracleFreshness(oracleTimestamp);
        
        if (!isFresh) {
            emit StaleOracleDetected(oracle, oracleTimestamp, block.timestamp, age);
            revert OracleDataStale(oracle, age, maxOracleStalenessPeriod);
        }
    }
    
    /**
     * ✅ AUDIT FIX #2: Validate cross-oracle synchronization
     * @notice Ensures all oracles in a trade are updated within acceptable time delta
     * @param oracles Array of oracle addresses
     * @param timestamps Array of oracle timestamps (must match oracles length)
     * @param maxDeltaSeconds Maximum allowed time difference between oracles
     */
    function _requireSynchronizedOracles(
        address[] memory oracles,
        uint256[] memory timestamps,
        uint256 maxDeltaSeconds
    ) internal view {
        require(oracles.length == timestamps.length, "OracleStalenessGuard: Length mismatch");
        require(oracles.length > 0, "OracleStalenessGuard: Empty oracle array");
        
        uint256 minTimestamp = type(uint256).max;
        uint256 maxTimestamp = 0;
        
        // First, validate each oracle individually
        for (uint256 i = 0; i < oracles.length; i++) {
            _requireFreshOracle(oracles[i], timestamps[i]);
            
            if (timestamps[i] < minTimestamp) {
                minTimestamp = timestamps[i];
            }
            if (timestamps[i] > maxTimestamp) {
                maxTimestamp = timestamps[i];
            }
        }
        
        // ✅ CRITICAL: Ensure all oracles updated within same time window
        uint256 delta = maxTimestamp - minTimestamp;
        require(delta <= maxDeltaSeconds, "OracleStalenessGuard: Oracle desync detected");
        
        emit OraclesSynchronized(oracles, delta);
    }
    
    /// @notice Event emitted when oracles pass synchronization check
    event OraclesSynchronized(address[] oracles, uint256 timeDelta);
    
    /**
     * @notice Check oracle against configured heartbeat
     * @param oracle Oracle address
     * @param oracleTimestamp Timestamp from oracle
     */
    function _requireWithinHeartbeat(address oracle, uint256 oracleTimestamp) internal view {
        uint256 heartbeat = oracleHeartbeats[oracle];
        
        if (heartbeat == 0) {
            // No specific heartbeat configured, use default check
            _requireFreshOracle(oracle, oracleTimestamp);
            return;
        }
        
        require(oracleTimestamp > 0, "Invalid oracle timestamp");
        require(oracleTimestamp <= block.timestamp, "Oracle timestamp in future");
        
        uint256 age = block.timestamp - oracleTimestamp;
        
        // Allow up to 2x heartbeat as safety margin
        if (age > heartbeat * 2) {
            emit StaleOracleDetected(oracle, oracleTimestamp, block.timestamp, age);
            revert OracleDataStale(oracle, age, heartbeat * 2);
        }
    }
    
    /**
     * @notice Validate Chainlink oracle response
     * @param roundId Chainlink round ID
     * @param answer Price from oracle
     * @param updatedAt Timestamp of last update
     * @param answeredInRound Round in which answer was computed
     */
    function _validateChainlinkResponse(
        uint80 roundId,
        int256 answer,
        uint256 updatedAt,
        uint80 answeredInRound
    ) internal view {
        require(answer > 0, "Invalid oracle price");
        require(roundId > 0, "Invalid round ID");
        require(updatedAt > 0, "Invalid update timestamp");
        require(answeredInRound >= roundId, "Stale round data");
        
        // Check staleness
        _requireFreshOracle(address(this), updatedAt);
    }
    
    // Custom errors (Solidity 0.8+)
    error OracleDataStale(address oracle, uint256 age, uint256 maxAge);
    error InvalidOracleTimestamp(uint256 timestamp);
}

/**
 * @title ChainlinkOracleConsumer
 * @notice Helper for consuming Chainlink price feeds with staleness checks
 */
interface IChainlinkOracle {
    function latestRoundData()
        external
        view
        returns (
            uint80 roundId,
            int256 answer,
            uint256 startedAt,
            uint256 updatedAt,
            uint80 answeredInRound
        );
}

/**
 * @title SafeOracleConsumer
 * @notice Example implementation of safe oracle consumption
 */
contract SafeOracleConsumer is OracleStalenessGuard {
    /**
     * @notice Get price from Chainlink oracle with staleness check
     * @param oracle Chainlink price feed address
     * @return price Latest price (scaled by oracle decimals)
     */
    function getChainlinkPrice(address oracle) public view returns (uint256 price) {
        require(oracle != address(0), "Invalid oracle address");
        
        (
            uint80 roundId,
            int256 answer,
            ,
            uint256 updatedAt,
            uint80 answeredInRound
        ) = IChainlinkOracle(oracle).latestRoundData();
        
        // ✅ AUDIT FIX: Validate staleness
        _validateChainlinkResponse(roundId, answer, updatedAt, answeredInRound);
        
        return uint256(answer);
    }
    
    /**
     * @notice Get price with custom staleness check
     * @param oracle Oracle address
     * @param maxAge Maximum allowed age in seconds
     */
    function getPriceWithCustomStaleness(address oracle, uint256 maxAge) 
        public 
        view 
        returns (uint256 price) 
    {
        (
            ,
            int256 answer,
            ,
            uint256 updatedAt,
            
        ) = IChainlinkOracle(oracle).latestRoundData();
        
        require(answer > 0, "Invalid price");
        require(block.timestamp - updatedAt < maxAge, "Price too stale");
        
        return uint256(answer);
    }
}

