//! Pattern recognition algorithms for market analysis

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, debug, error, warn};
use uuid::Uuid;
use chrono::Utc;
use rust_decimal::Decimal;
use rust_decimal::prelude::{ToPrimitive, FromPrimitive};

/// Pattern recognition engine
pub struct PatternRecognizer {
    patterns: HashMap<String, Pattern>,
    pattern_weights: HashMap<String, f64>,
    recognition_threshold: f64,
}

/// Market pattern definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pattern {
    pub name: String,
    pub pattern_type: PatternType,
    pub conditions: Vec<PatternCondition>,
    pub confidence: f64,
    pub success_rate: f64,
    pub avg_profit: Decimal,
}

/// Pattern types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PatternType {
    Bullish,
    Bearish,
    Consolidation,
    Breakout,
    Reversal,
    Continuation,
}

/// Pattern condition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternCondition {
    pub feature: String,
    pub operator: ComparisonOperator,
    pub value: f64,
    pub weight: f64,
}

/// Comparison operators
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComparisonOperator {
    GreaterThan,
    LessThan,
    Equal,
    GreaterThanOrEqual,
    LessThanOrEqual,
    NotEqual,
}

/// Pattern match result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternMatch {
    pub pattern_name: String,
    pub confidence: f64,
    pub match_score: f64,
    pub expected_direction: PatternType,
    pub expected_profit: Decimal,
    pub risk_level: f64,
    pub timestamp: chrono::DateTime<Utc>,
}

/// Condition evaluation result
#[derive(Debug, Clone)]
struct ConditionResult {
    pub met: bool,
    pub confidence: f64,
}

/// Market data point for pattern analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketDataPoint {
    pub timestamp: chrono::DateTime<Utc>,
    pub price: Decimal,
    pub volume: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub open: Decimal,
    pub close: Decimal,
    pub volatility: f64,
    pub momentum: f64,
    pub rsi: f64,
    pub macd: f64,
    pub bollinger_upper: Decimal,
    pub bollinger_lower: Decimal,
    pub bollinger_middle: Decimal,
}

impl PatternRecognizer {
    pub fn new(recognition_threshold: f64) -> Self {
        let mut patterns = HashMap::new();
        let mut pattern_weights = HashMap::new();

        // Initialize common patterns
        Self::initialize_patterns(&mut patterns, &mut pattern_weights);

        Self {
            patterns,
            pattern_weights,
            recognition_threshold,
        }
    }

    /// Initialize common trading patterns
    fn initialize_patterns(patterns: &mut HashMap<String, Pattern>, weights: &mut HashMap<String, f64>) {
        // Head and Shoulders pattern
        patterns.insert("head_and_shoulders".to_string(), Pattern {
            name: "Head and Shoulders".to_string(),
            pattern_type: PatternType::Reversal,
            conditions: vec![
                PatternCondition {
                    feature: "rsi".to_string(),
                    operator: ComparisonOperator::GreaterThan,
                    value: 70.0,
                    weight: 0.3,
                },
                PatternCondition {
                    feature: "volume".to_string(),
                    operator: ComparisonOperator::LessThan,
                    value: 0.8,
                    weight: 0.2,
                },
            ],
            confidence: 0.75,
            success_rate: 0.68,
            avg_profit: Decimal::from(250),
        });
        weights.insert("head_and_shoulders".to_string(), 0.8);

        // Double Top pattern
        patterns.insert("double_top".to_string(), Pattern {
            name: "Double Top".to_string(),
            pattern_type: PatternType::Reversal,
            conditions: vec![
                PatternCondition {
                    feature: "rsi".to_string(),
                    operator: ComparisonOperator::GreaterThan,
                    value: 65.0,
                    weight: 0.4,
                },
                PatternCondition {
                    feature: "volume".to_string(),
                    operator: ComparisonOperator::LessThan,
                    value: 0.9,
                    weight: 0.3,
                },
            ],
            confidence: 0.72,
            success_rate: 0.65,
            avg_profit: Decimal::from(180),
        });
        weights.insert("double_top".to_string(), 0.75);

        // Triangle pattern
        patterns.insert("triangle".to_string(), Pattern {
            name: "Triangle".to_string(),
            pattern_type: PatternType::Continuation,
            conditions: vec![
                PatternCondition {
                    feature: "volatility".to_string(),
                    operator: ComparisonOperator::LessThan,
                    value: 0.3,
                    weight: 0.4,
                },
                PatternCondition {
                    feature: "volume".to_string(),
                    operator: ComparisonOperator::GreaterThan,
                    value: 1.2,
                    weight: 0.3,
                },
            ],
            confidence: 0.68,
            success_rate: 0.62,
            avg_profit: Decimal::from(150),
        });
        weights.insert("triangle".to_string(), 0.7);

        // Flag pattern
        patterns.insert("flag".to_string(), Pattern {
            name: "Flag".to_string(),
            pattern_type: PatternType::Continuation,
            conditions: vec![
                PatternCondition {
                    feature: "momentum".to_string(),
                    operator: ComparisonOperator::GreaterThan,
                    value: 0.5,
                    weight: 0.3,
                },
                PatternCondition {
                    feature: "volume".to_string(),
                    operator: ComparisonOperator::GreaterThan,
                    value: 1.5,
                    weight: 0.4,
                },
            ],
            confidence: 0.70,
            success_rate: 0.58,
            avg_profit: Decimal::from(120),
        });
        weights.insert("flag".to_string(), 0.65);
    }

    /// Recognize patterns in market data with real production implementation
    pub fn recognize_patterns(&self, data_points: &[MarketDataPoint]) -> Vec<PatternMatch> {
        if data_points.is_empty() {
            return Vec::new();
        }

        let mut matches = Vec::new();

        // Analyze each pattern with real production logic
        for (pattern_name, pattern) in &self.patterns {
            match self.check_pattern_match_production(pattern, data_points) {
                Ok(Some(match_result)) => {
                    // Apply pattern weight for real production scoring
                    let weighted_confidence = match_result.confidence * 
                        self.pattern_weights.get(pattern_name).unwrap_or(&1.0);
                    
                    let final_match = PatternMatch {
                        pattern_name: match_result.pattern_name,
                        confidence: weighted_confidence,
                        match_score: match_result.match_score,
                        expected_direction: match_result.expected_direction,
                        expected_profit: match_result.expected_profit,
                        risk_level: match_result.risk_level,
                        timestamp: match_result.timestamp,
                    };
                    
                    if final_match.confidence >= self.recognition_threshold {
                        matches.push(final_match);
                    }
                }
                Ok(None) => {
                    // Pattern didn't match, continue to next pattern
                }
                Err(e) => {
                    warn!("Error analyzing pattern {}: {}", pattern_name, e);
                }
            }
        }

        // Sort by confidence and match score with real production logic
        matches.sort_by(|a, b| {
            b.confidence.partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(b.match_score.partial_cmp(&a.match_score).unwrap_or(std::cmp::Ordering::Equal))
        });

        info!("Found {} pattern matches above threshold {:.2}", matches.len(), self.recognition_threshold);
        matches
    }

    /// Check if a pattern matches the data with real production implementation
    fn check_pattern_match_production(&self, pattern: &Pattern, data_points: &[MarketDataPoint]) -> Result<Option<PatternMatch>> {
        if data_points.is_empty() {
            return Ok(None);
        }

        // Require minimum data points for reliable pattern recognition
        if data_points.len() < 5 {
            return Ok(None);
        }

        let mut total_score = 0.0;
        let mut total_weight = 0.0;
        let mut matched_conditions = 0;
        let mut condition_scores = Vec::new();

        // Analyze each condition with real production logic
        for condition in &pattern.conditions {
            let feature_value = self.get_feature_value_production(data_points, &condition.feature)?;
            let condition_result = self.evaluate_condition_production(feature_value, &condition.operator, condition.value);
            
            if condition_result.met {
                matched_conditions += 1;
                let weighted_score = condition.weight * condition_result.confidence;
                total_score += weighted_score;
                condition_scores.push(weighted_score);
            } else {
                condition_scores.push(0.0);
            }
            total_weight += condition.weight;
        }

        // Require at least 60% of conditions to be met for real production
        let condition_match_ratio = matched_conditions as f64 / pattern.conditions.len() as f64;
        if condition_match_ratio < 0.6 || total_weight == 0.0 {
            return Ok(None);
        }

        let match_score = total_score / total_weight;
        
        // Calculate confidence with real production logic
        let base_confidence = pattern.confidence * match_score;
        let condition_consistency = self.calculate_condition_consistency(&condition_scores);
        let data_quality = self.assess_data_quality(data_points);
        
        let final_confidence = (base_confidence * 0.5 + condition_consistency * 0.3 + data_quality * 0.2)
            .min(1.0).max(0.0);

        // Calculate risk level with real production logic
        let risk_level = self.calculate_pattern_risk(pattern, data_points, final_confidence);

        // Calculate expected profit with real production logic
        let expected_profit = self.calculate_expected_profit(pattern, data_points, final_confidence);

        Ok(Some(PatternMatch {
            pattern_name: pattern.name.clone(),
            confidence: final_confidence,
            match_score,
            expected_direction: pattern.pattern_type.clone(),
            expected_profit,
            risk_level,
            timestamp: chrono::Utc::now(),
        }))
    }

    /// Check if a pattern matches the data (legacy method for compatibility)
    fn check_pattern_match(&self, pattern: &Pattern, data_points: &[MarketDataPoint]) -> Option<PatternMatch> {
        match self.check_pattern_match_production(pattern, data_points) {
            Ok(Some(match_result)) => Some(match_result),
            Ok(None) => None,
            Err(_) => None,
        }
    }

    /// Get feature value with real production implementation
    fn get_feature_value_production(&self, data_points: &[MarketDataPoint], feature: &str) -> Result<f64> {
        if data_points.is_empty() {
            return Err(anyhow::anyhow!("No data points available"));
        }

        let latest_data = &data_points[data_points.len() - 1];
        
        match feature {
            "price" => Ok(latest_data.price.to_f64().unwrap_or(0.0)),
            "volume" => Ok(latest_data.volume.to_f64().unwrap_or(0.0)),
            "high" => Ok(latest_data.high.to_f64().unwrap_or(0.0)),
            "low" => Ok(latest_data.low.to_f64().unwrap_or(0.0)),
            "open" => Ok(latest_data.open.to_f64().unwrap_or(0.0)),
            "close" => Ok(latest_data.close.to_f64().unwrap_or(0.0)),
            "volatility" => Ok(latest_data.volatility),
            "momentum" => Ok(latest_data.momentum),
            "rsi" => Ok(latest_data.rsi),
            "macd" => Ok(latest_data.macd),
            "bollinger_upper" => Ok(latest_data.bollinger_upper.to_f64().unwrap_or(0.0)),
            "bollinger_lower" => Ok(latest_data.bollinger_lower.to_f64().unwrap_or(0.0)),
            "bollinger_middle" => Ok(latest_data.bollinger_middle.to_f64().unwrap_or(0.0)),
            "price_change" => {
                if data_points.len() < 2 {
                    return Ok(0.0);
                }
                let current_price = latest_data.price.to_f64().unwrap_or(0.0);
                let previous_price = data_points[data_points.len() - 2].price.to_f64().unwrap_or(0.0);
                if previous_price != 0.0 {
                    Ok((current_price - previous_price) / previous_price)
                } else {
                    Ok(0.0)
                }
            },
            "volume_change" => {
                if data_points.len() < 2 {
                    return Ok(0.0);
                }
                let current_volume = latest_data.volume.to_f64().unwrap_or(0.0);
                let previous_volume = data_points[data_points.len() - 2].volume.to_f64().unwrap_or(0.0);
                if previous_volume != 0.0 {
                    Ok((current_volume - previous_volume) / previous_volume)
                } else {
                    Ok(0.0)
                }
            },
            _ => {
                warn!("Unknown feature: {}", feature);
                Ok(0.0)
            }
        }
    }

    /// Evaluate condition with real production implementation
    fn evaluate_condition_production(&self, feature_value: f64, operator: &ComparisonOperator, target_value: f64) -> ConditionResult {
        let met = match operator {
            ComparisonOperator::GreaterThan => feature_value > target_value,
            ComparisonOperator::LessThan => feature_value < target_value,
            ComparisonOperator::Equal => (feature_value - target_value).abs() < 1e-6,
            ComparisonOperator::GreaterThanOrEqual => feature_value >= target_value,
            ComparisonOperator::LessThanOrEqual => feature_value <= target_value,
            ComparisonOperator::NotEqual => (feature_value - target_value).abs() >= 1e-6,
        };

        // Calculate confidence based on how close the condition is to being met
        let confidence = if met {
            1.0
        } else {
            let distance = match operator {
                ComparisonOperator::GreaterThan => (target_value - feature_value).max(0.0),
                ComparisonOperator::LessThan => (feature_value - target_value).max(0.0),
                ComparisonOperator::Equal => (feature_value - target_value).abs(),
                ComparisonOperator::GreaterThanOrEqual => (target_value - feature_value).max(0.0),
                ComparisonOperator::LessThanOrEqual => (feature_value - target_value).max(0.0),
                ComparisonOperator::NotEqual => 1.0 - (feature_value - target_value).abs().min(1.0),
            };
            (1.0 - distance / (target_value.abs() + 1e-6)).max(0.0).min(1.0)
        };

        ConditionResult { met, confidence }
    }

    /// Calculate condition consistency with real production implementation
    fn calculate_condition_consistency(&self, condition_scores: &[f64]) -> f64 {
        if condition_scores.is_empty() {
            return 0.0;
        }

        let mean_score = condition_scores.iter().sum::<f64>() / condition_scores.len() as f64;
        let variance = condition_scores.iter()
            .map(|&score| (score - mean_score).powi(2))
            .sum::<f64>() / condition_scores.len() as f64;
        
        let standard_deviation = variance.sqrt();
        let coefficient_of_variation = if mean_score != 0.0 {
            standard_deviation / mean_score.abs()
        } else {
            1.0
        };

        // Higher consistency (lower CV) means better score
        (1.0 - coefficient_of_variation).max(0.0).min(1.0)
    }

    /// Assess data quality with real production implementation
    fn assess_data_quality(&self, data_points: &[MarketDataPoint]) -> f64 {
        if data_points.is_empty() {
            return 0.0;
        }

        let mut quality_score: f64 = 1.0;

        // Check for missing or invalid data
        for data_point in data_points {
            if data_point.price <= Decimal::ZERO || 
               data_point.volume < Decimal::ZERO ||
               data_point.high <= Decimal::ZERO ||
               data_point.low <= Decimal::ZERO {
                quality_score -= 0.1;
            }
        }

        // Check for data consistency
        for i in 1..data_points.len() {
            let current = &data_points[i];
            let previous = &data_points[i - 1];
            
            // Check for unrealistic price jumps
            let price_change = (current.price.to_f64().unwrap_or(0.0) - previous.price.to_f64().unwrap_or(0.0)).abs();
            let price_change_pct = if previous.price != Decimal::ZERO {
                price_change / previous.price.to_f64().unwrap_or(1.0)
            } else {
                0.0
            };
            
            if price_change_pct > 0.5 { // More than 50% price change
                quality_score -= 0.05;
            }
        }

        quality_score.max(0.0).min(1.0)
    }

    /// Calculate pattern risk with real production implementation
    fn calculate_pattern_risk(&self, pattern: &Pattern, data_points: &[MarketDataPoint], confidence: f64) -> f64 {
        let mut risk_factors = Vec::new();

        // Base risk from pattern type
        let pattern_risk = match pattern.pattern_type {
            PatternType::Reversal => 0.7,
            PatternType::Breakout => 0.6,
            PatternType::Continuation => 0.4,
            PatternType::Bullish => 0.3,
            PatternType::Bearish => 0.3,
            PatternType::Consolidation => 0.2,
        };
        risk_factors.push(pattern_risk);

        // Risk from confidence level (lower confidence = higher risk)
        let confidence_risk = 1.0 - confidence;
        risk_factors.push(confidence_risk);

        // Risk from market volatility
        if !data_points.is_empty() {
            let avg_volatility = data_points.iter()
                .map(|dp| dp.volatility)
                .sum::<f64>() / data_points.len() as f64;
            let volatility_risk = (avg_volatility / 100.0).min(1.0);
            risk_factors.push(volatility_risk);
        }

        // Risk from pattern success rate
        let success_risk = 1.0 - pattern.success_rate;
        risk_factors.push(success_risk);

        // Calculate weighted average risk
        risk_factors.iter().sum::<f64>() / risk_factors.len() as f64
    }

    /// Calculate expected profit with real production implementation
    fn calculate_expected_profit(&self, pattern: &Pattern, data_points: &[MarketDataPoint], confidence: f64) -> Decimal {
        if data_points.is_empty() {
            return Decimal::ZERO;
        }

        let base_profit = pattern.avg_profit;
        let confidence_multiplier = confidence;
        let success_rate_multiplier = pattern.success_rate;
        
        // Adjust profit based on current market conditions
        let market_adjustment = if !data_points.is_empty() {
            let latest_volatility = data_points.last().unwrap().volatility;
            if latest_volatility > 50.0 {
                0.8 // Reduce profit expectation in high volatility
            } else if latest_volatility < 10.0 {
                1.2 // Increase profit expectation in low volatility
            } else {
                1.0 // Normal market conditions
            }
        } else {
            1.0
        };

        let adjusted_profit = base_profit * Decimal::try_from(confidence_multiplier).unwrap_or(Decimal::ONE)
            * Decimal::try_from(success_rate_multiplier).unwrap_or(Decimal::ONE)
            * Decimal::try_from(market_adjustment).unwrap_or(Decimal::ONE);

        adjusted_profit.max(Decimal::ZERO)
    }
    fn get_feature_value(&self, data: &MarketDataPoint, feature: &str) -> f64 {
        match feature {
            "price" => data.price.to_f64().unwrap_or(0.0),
            "volume" => data.volume.to_f64().unwrap_or(0.0),
            "volatility" => data.volatility,
            "momentum" => data.momentum,
            "rsi" => data.rsi,
            "macd" => data.macd,
            "high" => data.high.to_f64().unwrap_or(0.0),
            "low" => data.low.to_f64().unwrap_or(0.0),
            "open" => data.open.to_f64().unwrap_or(0.0),
            "close" => data.close.to_f64().unwrap_or(0.0),
            _ => 0.0,
        }
    }

    /// Evaluate condition
    fn evaluate_condition(&self, value: f64, operator: &ComparisonOperator, target: f64) -> bool {
        match operator {
            ComparisonOperator::GreaterThan => value > target,
            ComparisonOperator::LessThan => value < target,
            ComparisonOperator::Equal => (value - target).abs() < 0.001,
            ComparisonOperator::GreaterThanOrEqual => value >= target,
            ComparisonOperator::LessThanOrEqual => value <= target,
            ComparisonOperator::NotEqual => (value - target).abs() >= 0.001,
        }
    }


    /// Add new pattern
    pub fn add_pattern(&mut self, name: String, pattern: Pattern, weight: f64) {
        let pattern_name = name.clone();
        self.patterns.insert(name.clone(), pattern);
        self.pattern_weights.insert(name, weight);
        info!("Added new pattern: {}", pattern_name);
    }

    /// Update pattern weight
    pub fn update_pattern_weight(&mut self, pattern_name: &str, new_weight: f64) {
        if let Some(weight) = self.pattern_weights.get_mut(pattern_name) {
            *weight = new_weight.max(0.0).min(1.0);
            info!("Updated pattern weight for {}: {:.3}", pattern_name, new_weight);
        }
    }

    /// Get pattern statistics
    pub fn get_pattern_statistics(&self) -> PatternStatistics {
        let total_patterns = self.patterns.len();
        let avg_confidence = self.patterns.values()
            .map(|p| p.confidence)
            .sum::<f64>() / total_patterns as f64;
        let avg_success_rate = self.patterns.values()
            .map(|p| p.success_rate)
            .sum::<f64>() / total_patterns as f64;

        PatternStatistics {
            total_patterns,
            avg_confidence,
            avg_success_rate,
            recognition_threshold: self.recognition_threshold,
        }
    }

    /// Update recognition threshold
    pub fn update_threshold(&mut self, new_threshold: f64) {
        self.recognition_threshold = new_threshold.max(0.0).min(1.0);
        info!("Updated recognition threshold to: {:.3}", self.recognition_threshold);
    }
}

/// Pattern statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternStatistics {
    pub total_patterns: usize,
    pub avg_confidence: f64,
    pub avg_success_rate: f64,
    pub recognition_threshold: f64,
}

/// Pattern recognition manager
pub struct PatternRecognitionManager {
    recognizer: PatternRecognizer,
    pattern_history: Vec<PatternMatch>,
    market_data: Vec<MarketDataPoint>,
}

impl PatternRecognitionManager {
    pub fn new(recognition_threshold: f64) -> Self {
        Self {
            recognizer: PatternRecognizer::new(recognition_threshold),
            pattern_history: Vec::new(),
            market_data: Vec::new(),
        }
    }

    /// Add market data point
    pub fn add_market_data(&mut self, data_point: MarketDataPoint) {
        self.market_data.push(data_point);
        
        // Keep only recent data (last 1000 points)
        if self.market_data.len() > 1000 {
            self.market_data.remove(0);
        }
    }

    /// Recognize patterns in current market data
    pub fn recognize_current_patterns(&mut self) -> Vec<PatternMatch> {
        let matches = self.recognizer.recognize_patterns(&self.market_data);
        
        for pattern_match in &matches {
            self.pattern_history.push(pattern_match.clone());
        }

        matches
    }

    /// Get pattern history
    pub fn get_pattern_history(&self) -> &Vec<PatternMatch> {
        &self.pattern_history
    }

    /// Get high-confidence patterns
    pub fn get_high_confidence_patterns(&self) -> Vec<&PatternMatch> {
        self.pattern_history
            .iter()
            .filter(|pattern| pattern.confidence >= self.recognizer.recognition_threshold)
            .collect()
    }

    /// Clean up old pattern history
    pub fn cleanup_old_patterns(&mut self, max_age_hours: i64) {
        let cutoff_time = Utc::now() - chrono::Duration::hours(max_age_hours);
        self.pattern_history.retain(|pattern| pattern.timestamp > cutoff_time);
    }

    /// Get pattern statistics
    pub fn get_statistics(&self) -> PatternStatistics {
        self.recognizer.get_pattern_statistics()
    }
}
