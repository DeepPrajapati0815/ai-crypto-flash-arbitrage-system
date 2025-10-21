//! Historical data management for backtesting

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, debug, error, warn};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::{ToPrimitive, FromPrimitive};
use csv::Reader;

use crate::core::types::TradingPair;
use super::engine::MarketDataSnapshot;

/// Historical data manager
pub struct HistoricalData {
    data: HashMap<String, Vec<MarketDataSnapshot>>, // exchange_pair -> data points
    start_time: Option<DateTime<Utc>>,
    end_time: Option<DateTime<Utc>>,
}

/// Data source configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSource {
    pub name: String,
    pub file_path: String,
    pub format: DataFormat,
    pub delimiter: char,
    pub has_header: bool,
    pub time_column: String,
    pub price_columns: HashMap<String, String>, // field -> column name
}

/// Data format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataFormat {
    CSV,
    JSON,
    Parquet,
    Database,
}

/// Data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataPoint {
    pub timestamp: DateTime<Utc>,
    pub exchange: String,
    pub pair: TradingPair,
    pub bid_price: Decimal,
    pub ask_price: Decimal,
    pub last_price: Decimal,
    pub volume_24h: Decimal,
    pub high_24h: Decimal,
    pub low_24h: Decimal,
    pub open_24h: Decimal,
}

impl HistoricalData {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
            start_time: None,
            end_time: None,
        }
    }

    /// Load data from CSV file
    pub async fn load_from_csv(&mut self, file_path: &str) -> Result<()> {
        info!("Loading historical data from CSV: {}", file_path);
        
        let content = tokio::fs::read_to_string(file_path).await?;
        let mut rdr = Reader::from_reader(content.as_bytes());
        
        let mut records = Vec::new();
        for result in rdr.deserialize() {
            let record: DataPoint = result?;
            records.push(record);
        }
        
        // Group data by exchange_pair
        for record in records {
            let key = format!("{}_{}", record.exchange, record.pair.symbol());
            self.data.entry(key).or_insert_with(Vec::new).push(MarketDataSnapshot {
                timestamp: record.timestamp,
                exchange: record.exchange,
                pair: record.pair,
                bid_price: record.bid_price,
                ask_price: record.ask_price,
                last_price: record.last_price,
                volume_24h: record.volume_24h,
            });
        }
        
        // Sort data by timestamp
        for (_, data_points) in self.data.iter_mut() {
            data_points.sort_by_key(|dp| dp.timestamp);
        }
        
        // Update time range
        if let Some(first_time) = self.data.values()
            .flat_map(|v| v.first())
            .map(|dp| dp.timestamp)
            .min() {
            self.start_time = Some(first_time);
        }
        
        if let Some(last_time) = self.data.values()
            .flat_map(|v| v.last())
            .map(|dp| dp.timestamp)
            .max() {
            self.end_time = Some(last_time);
        }
        
        info!("Loaded {} data points from {} to {}", 
              self.get_data_count(), 
              self.start_time.unwrap_or(Utc::now()),
              self.end_time.unwrap_or(Utc::now()));
        
        Ok(())
    }

    /// Load data from multiple CSV files
    pub async fn load_from_multiple_csv(&mut self, file_paths: &[String]) -> Result<()> {
        for file_path in file_paths {
            self.load_from_csv(file_path).await?;
        }
        Ok(())
    }

    /// Load data from database
    pub async fn load_from_database(&mut self, connection_string: &str, query: &str) -> Result<()> {
        info!("Loading historical data from database");
        
        // TODO: Implement database loading
        // This would typically use sqlx to query a database
        
        warn!("Database loading not yet implemented");
        Ok(())
    }

    /// Get data at a specific time
    pub async fn get_data_at_time(&self, time: DateTime<Utc>) -> Result<HashMap<String, MarketDataSnapshot>> {
        let mut result = HashMap::new();
        
        for (key, data_points) in &self.data {
            // Find the closest data point to the requested time
            if let Some(closest_point) = self.find_closest_data_point(data_points, time) {
                result.insert(key.clone(), closest_point);
            }
        }
        
        Ok(result)
    }

    /// Get data in a time range
    pub async fn get_data_in_range(&self, start_time: DateTime<Utc>, end_time: DateTime<Utc>) -> Result<HashMap<String, Vec<MarketDataSnapshot>>> {
        let mut result = HashMap::new();
        
        for (key, data_points) in &self.data {
            let filtered_points: Vec<MarketDataSnapshot> = data_points
                .iter()
                .filter(|dp| dp.timestamp >= start_time && dp.timestamp <= end_time)
                .cloned()
                .collect();
            
            if !filtered_points.is_empty() {
                result.insert(key.clone(), filtered_points);
            }
        }
        
        Ok(result)
    }

    /// Find closest data point to a given time
    fn find_closest_data_point(&self, data_points: &[MarketDataSnapshot], target_time: DateTime<Utc>) -> Option<MarketDataSnapshot> {
        if data_points.is_empty() {
            return None;
        }
        
        // Binary search for the closest point
        let mut left = 0;
        let mut right = data_points.len() - 1;
        
        while left <= right {
            let mid = (left + right) / 2;
            let mid_time = data_points[mid].timestamp;
            
            if mid_time == target_time {
                return Some(data_points[mid].clone());
            } else if mid_time < target_time {
                left = mid + 1;
            } else {
                right = mid - 1;
            }
        }
        
        // Find the closest point among the candidates
        let mut candidates = if left < data_points.len() {
            vec![left]
        } else {
            vec![right]
        };
        
        if right > 0 {
            candidates.push(right - 1);
        }
        
        candidates
            .into_iter()
            .filter(|&i| i < data_points.len())
            .min_by_key(|&i| {
                let diff = (data_points[i].timestamp - target_time).num_milliseconds().abs();
                diff
            })
            .map(|i| data_points[i].clone())
    }

    /// Get data count
    pub fn get_data_count(&self) -> usize {
        self.data.values().map(|v| v.len()).sum()
    }

    /// Get time range
    pub fn get_time_range(&self) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
        if let (Some(start), Some(end)) = (self.start_time, self.end_time) {
            Some((start, end))
        } else {
            None
        }
    }

    /// Get available exchanges
    pub fn get_exchanges(&self) -> Vec<String> {
        self.data.keys()
            .map(|key| key.split('_').next().unwrap_or("").to_string())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect()
    }

    /// Get available pairs
    pub fn get_pairs(&self) -> Vec<TradingPair> {
        self.data.keys()
            .filter_map(|key| {
                let parts: Vec<&str> = key.split('_').collect();
                if parts.len() >= 2 {
                    let base = parts[1];
                    let quote = if parts.len() > 2 { parts[2] } else { "USDT" };
                    Some(TradingPair::new(base, quote))
                } else {
                    None
                }
            })
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect()
    }

    /// Validate data integrity
    pub fn validate_data(&self) -> Result<()> {
        info!("Validating historical data integrity");
        
        let mut issues = Vec::new();
        
        for (key, data_points) in &self.data {
            if data_points.is_empty() {
                issues.push(format!("No data points for {}", key));
                continue;
            }
            
            // Check for duplicate timestamps
            let mut timestamps = data_points.iter().map(|dp| dp.timestamp).collect::<Vec<_>>();
            timestamps.sort();
            let unique_timestamps = timestamps.len();
            let total_timestamps = data_points.len();
            
            if unique_timestamps != total_timestamps {
                issues.push(format!("Duplicate timestamps found for {}", key));
            }
            
            // Check for negative prices
            for (i, dp) in data_points.iter().enumerate() {
                if dp.bid_price <= Decimal::ZERO || dp.ask_price <= Decimal::ZERO || dp.last_price <= Decimal::ZERO {
                    issues.push(format!("Invalid price at index {} for {}", i, key));
                }
                
                if dp.bid_price > dp.ask_price {
                    issues.push(format!("Bid price > Ask price at index {} for {}", i, key));
                }
            }
        }
        
        if !issues.is_empty() {
            error!("Data validation failed:");
            for issue in &issues {
                error!("  {}", issue);
            }
            return Err(anyhow::anyhow!("Data validation failed: {} issues found", issues.len()));
        }
        
        info!("Data validation passed");
        Ok(())
    }

    /// Clean and normalize data
    pub async fn clean_data(&mut self) -> Result<()> {
        info!("Cleaning and normalizing historical data");
        
        for (_, data_points) in self.data.iter_mut() {
            // Remove duplicates
            data_points.sort_by_key(|dp| dp.timestamp);
            data_points.dedup_by_key(|dp| dp.timestamp);
            
            // Remove outliers (prices that are more than 3 standard deviations from mean)
            Self::remove_outliers(data_points);
            
            // Fill missing data points with interpolation
            Self::interpolate_missing_data(data_points);
        }
        
        info!("Data cleaning completed");
        Ok(())
    }

    /// Remove outliers from data
    fn remove_outliers(data_points: &mut Vec<MarketDataSnapshot>) {
        if data_points.len() < 3 {
            return;
        }
        
        // Calculate mean and standard deviation for last_price
        let prices: Vec<f64> = data_points.iter()
            .map(|dp| dp.last_price.to_f64().unwrap_or(0.0))
            .collect();
        
        let mean = prices.iter().sum::<f64>() / prices.len() as f64;
        let variance = prices.iter()
            .map(|&x| (x - mean).powi(2))
            .sum::<f64>() / prices.len() as f64;
        let std_dev = variance.sqrt();
        
        let threshold = 3.0 * std_dev;
        
        data_points.retain(|dp| {
            if let Some(price) = dp.last_price.to_f64() {
                (price - mean).abs() <= threshold
            } else {
                true
            }
        });
    }

    /// Interpolate missing data points
    fn interpolate_missing_data(data_points: &mut Vec<MarketDataSnapshot>) {
        if data_points.len() < 2 {
            return;
        }
        
        // Sort by timestamp
        data_points.sort_by_key(|dp| dp.timestamp);
        
        let mut interpolated = Vec::new();
        let mut i = 0;
        
        while i < data_points.len() - 1 {
            let current = &data_points[i];
            let next = &data_points[i + 1];
            
            interpolated.push(current.clone());
            
            // Check if there's a gap larger than 1 minute
            let gap = next.timestamp - current.timestamp;
            if gap.num_minutes() > 1 {
                // Interpolate missing points
                let steps = gap.num_minutes() as usize;
                for step in 1..steps {
                    let interpolated_time = current.timestamp + chrono::Duration::minutes(step as i64);
                    let ratio = step as f64 / steps as f64;
                    
                    let interpolated_point = MarketDataSnapshot {
                        timestamp: interpolated_time,
                        exchange: current.exchange.clone(),
                        pair: current.pair.clone(),
                        bid_price: Self::interpolate_decimal(current.bid_price, next.bid_price, ratio),
                        ask_price: Self::interpolate_decimal(current.ask_price, next.ask_price, ratio),
                        last_price: Self::interpolate_decimal(current.last_price, next.last_price, ratio),
                        volume_24h: Self::interpolate_decimal(current.volume_24h, next.volume_24h, ratio),
                    };
                    
                    interpolated.push(interpolated_point);
                }
            }
            
            i += 1;
        }
        
        // Add the last point
        if let Some(last) = data_points.last() {
            interpolated.push(last.clone());
        }
        
        *data_points = interpolated;
    }

    /// Interpolate between two decimal values
    fn interpolate_decimal(start: Decimal, end: Decimal, ratio: f64) -> Decimal {
        let start_f64 = start.to_f64().unwrap_or(0.0);
        let end_f64 = end.to_f64().unwrap_or(0.0);
        let interpolated = start_f64 + (end_f64 - start_f64) * ratio;
        Decimal::from_f64(interpolated).unwrap_or(start)
    }
}
