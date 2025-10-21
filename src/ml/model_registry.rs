//! ML Model Registry and Versioning System
//! 
//! Provides centralized model management with version control, A/B testing,
//! and performance tracking for machine learning models

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};
use tracing::{info, debug};

/// Model version identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModelVersion {
    pub model_name: String,
    pub version: String,
    pub created_at: DateTime<Utc>,
}

impl ModelVersion {
    pub fn new(model_name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            model_name: model_name.into(),
            version: version.into(),
            created_at: Utc::now(),
        }
    }
    
    pub fn to_string(&self) -> String {
        format!("{}::{}", self.model_name, self.version)
    }
}

/// Model metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetadata {
    pub version: ModelVersion,
    pub description: String,
    pub model_type: ModelType,
    pub framework: String,
    pub file_path: PathBuf,
    pub file_size_bytes: u64,
    pub checksum: String,
    pub hyperparameters: HashMap<String, String>,
    pub training_metrics: TrainingMetrics,
    pub deployed_at: Option<DateTime<Utc>>,
    pub deprecated_at: Option<DateTime<Utc>>,
    pub tags: Vec<String>,
}

/// Model type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelType {
    /// LSTM for price prediction
    LstmPredictor,
    /// GRU for volatility forecasting
    GruVolatility,
    /// Transformer for pattern recognition
    TransformerPattern,
    /// Ensemble model
    Ensemble,
    /// Reinforcement learning agent
    RLAgent,
    /// Statistical arbitrage
    StatisticalArb,
}

/// Training metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingMetrics {
    pub accuracy: f64,
    pub precision: f64,
    pub recall: f64,
    pub f1_score: f64,
    pub mae: f64,
    pub rmse: f64,
    pub training_time_seconds: f64,
    pub validation_loss: f64,
    pub training_samples: usize,
    pub validation_samples: usize,
}

impl Default for TrainingMetrics {
    fn default() -> Self {
        Self {
            accuracy: 0.0,
            precision: 0.0,
            recall: 0.0,
            f1_score: 0.0,
            mae: 0.0,
            rmse: 0.0,
            training_time_seconds: 0.0,
            validation_loss: 0.0,
            training_samples: 0,
            validation_samples: 0,
        }
    }
}

/// Production performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionMetrics {
    pub total_predictions: u64,
    pub correct_predictions: u64,
    pub average_confidence: f64,
    pub average_latency_ms: f64,
    pub profit_generated: f64,
    pub trades_triggered: u64,
    pub last_updated: DateTime<Utc>,
}

impl Default for ProductionMetrics {
    fn default() -> Self {
        Self {
            total_predictions: 0,
            correct_predictions: 0,
            average_confidence: 0.0,
            average_latency_ms: 0.0,
            profit_generated: 0.0,
            trades_triggered: 0,
            last_updated: Utc::now(),
        }
    }
}

/// Model deployment status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeploymentStatus {
    /// Model is staged but not deployed
    Staged,
    /// Model is in A/B testing
    ABTesting,
    /// Model is fully deployed in production
    Production,
    /// Model is being rolled back
    RollingBack,
    /// Model is deprecated
    Deprecated,
}

/// Model registry entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRegistryEntry {
    pub metadata: ModelMetadata,
    pub deployment_status: DeploymentStatus,
    pub production_metrics: ProductionMetrics,
    pub ab_test_config: Option<ABTestConfig>,
}

/// A/B testing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABTestConfig {
    pub test_name: String,
    pub control_version: String,
    pub treatment_version: String,
    pub traffic_split: f64, // 0.0-1.0, percentage to treatment
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub min_sample_size: usize,
    pub success_metric: String,
}

/// ML Model Registry
pub struct ModelRegistry {
    models: Arc<RwLock<HashMap<String, Vec<ModelRegistryEntry>>>>,
    active_models: Arc<RwLock<HashMap<String, String>>>, // model_name -> version
    storage_path: PathBuf,
}

impl ModelRegistry {
    /// Create a new model registry
    pub fn new(storage_path: impl Into<PathBuf>) -> Self {
        let storage_path = storage_path.into();
        info!("Initializing ML Model Registry at {:?}", storage_path);
        
        Self {
            models: Arc::new(RwLock::new(HashMap::new())),
            active_models: Arc::new(RwLock::new(HashMap::new())),
            storage_path,
        }
    }
    
    /// Register a new model version
    pub async fn register_model(
        &self,
        metadata: ModelMetadata,
        deployment_status: DeploymentStatus,
    ) -> Result<()> {
        let model_name = metadata.version.model_name.clone();
        let version = metadata.version.version.clone();
        
        info!("Registering model: {} version {}", model_name, version);
        
        let entry = ModelRegistryEntry {
            metadata,
            deployment_status,
            production_metrics: ProductionMetrics::default(),
            ab_test_config: None,
        };
        
        let mut models = self.models.write().await;
        models.entry(model_name.clone())
            .or_insert_with(Vec::new)
            .push(entry);
        
        // If this is the first model, make it active
        let mut active_models = self.active_models.write().await;
        if !active_models.contains_key(&model_name) {
            active_models.insert(model_name.clone(), version.clone());
            info!("Set {} version {} as active model", model_name, version);
        }
        
        Ok(())
    }
    
    /// Get active version of a model
    pub async fn get_active_version(&self, model_name: &str) -> Option<String> {
        self.active_models.read().await.get(model_name).cloned()
    }
    
    /// Set active version for a model
    pub async fn set_active_version(&self, model_name: &str, version: &str) -> Result<()> {
        // Verify version exists
        let models = self.models.read().await;
        let versions = models.get(model_name)
            .ok_or_else(|| anyhow::anyhow!("Model '{}' not found", model_name))?;
        
        if !versions.iter().any(|e| e.metadata.version.version == version) {
            return Err(anyhow::anyhow!(
                "Version '{}' not found for model '{}'",
                version, model_name
            ));
        }
        
        drop(models);
        
        let mut active_models = self.active_models.write().await;
        active_models.insert(model_name.to_string(), version.to_string());
        
        info!("Activated model {} version {}", model_name, version);
        Ok(())
    }
    
    /// Get model metadata
    pub async fn get_model_metadata(
        &self,
        model_name: &str,
        version: Option<&str>,
    ) -> Result<ModelMetadata> {
        let models = self.models.read().await;
        let versions = models.get(model_name)
            .ok_or_else(|| anyhow::anyhow!("Model '{}' not found", model_name))?;
        
        let target_version = match version {
            Some(v) => v.to_string(),
            None => {
                let active = self.active_models.read().await;
                active.get(model_name)
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("No active version for '{}'", model_name))?
            }
        };
        
        versions.iter()
            .find(|e| e.metadata.version.version == *target_version)
            .map(|e| e.metadata.clone())
            .ok_or_else(|| anyhow::anyhow!(
                "Version '{}' not found for model '{}'",
                target_version, model_name
            ))
    }
    
    /// List all versions of a model
    pub async fn list_versions(&self, model_name: &str) -> Vec<ModelVersion> {
        self.models.read().await
            .get(model_name)
            .map(|versions| {
                versions.iter()
                    .map(|e| e.metadata.version.clone())
                    .collect()
            })
            .unwrap_or_default()
    }
    
    /// List all registered models
    pub async fn list_models(&self) -> Vec<String> {
        self.models.read().await.keys().cloned().collect()
    }
    
    /// Update production metrics for a model
    pub async fn update_production_metrics(
        &self,
        model_name: &str,
        version: &str,
        metrics: ProductionMetrics,
    ) -> Result<()> {
        let mut models = self.models.write().await;
        let versions = models.get_mut(model_name)
            .ok_or_else(|| anyhow::anyhow!("Model '{}' not found", model_name))?;
        
        for entry in versions.iter_mut() {
            if entry.metadata.version.version == version {
                entry.production_metrics = metrics;
                debug!(
                    "Updated production metrics for {} version {}",
                    model_name, version
                );
                return Ok(());
            }
        }
        
        Err(anyhow::anyhow!(
            "Version '{}' not found for model '{}'",
            version, model_name
        ))
    }
    
    /// Get production metrics
    pub async fn get_production_metrics(
        &self,
        model_name: &str,
        version: Option<&str>,
    ) -> Result<ProductionMetrics> {
        let models = self.models.read().await;
        let versions = models.get(model_name)
            .ok_or_else(|| anyhow::anyhow!("Model '{}' not found", model_name))?;
        
        let target_version: String = match version {
            Some(v) => v.to_string(),
            None => {
                self.active_models.read().await
                    .get(model_name)
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("No active version for '{}'", model_name))?
            }
        };
        
        versions.iter()
            .find(|e| e.metadata.version.version == *target_version)
            .map(|e| e.production_metrics.clone())
            .ok_or_else(|| anyhow::anyhow!(
                "Version '{}' not found for model '{}'",
                target_version, model_name
            ))
    }
    
    /// Start A/B test for a model
    pub async fn start_ab_test(
        &self,
        model_name: &str,
        control_version: &str,
        treatment_version: &str,
        config: ABTestConfig,
    ) -> Result<()> {
        info!(
            "Starting A/B test for {}: {} vs {}",
            model_name, control_version, treatment_version
        );
        
        let mut models = self.models.write().await;
        let versions = models.get_mut(model_name)
            .ok_or_else(|| anyhow::anyhow!("Model '{}' not found", model_name))?;
        
        // Set both versions to AB testing status
        for entry in versions.iter_mut() {
            if entry.metadata.version.version == control_version
                || entry.metadata.version.version == treatment_version
            {
                entry.deployment_status = DeploymentStatus::ABTesting;
                entry.ab_test_config = Some(config.clone());
            }
        }
        
        Ok(())
    }
    
    /// End A/B test and promote winner
    pub async fn end_ab_test(&self, model_name: &str, winner_version: &str) -> Result<()> {
        info!("Ending A/B test for {}, promoting version {}", model_name, winner_version);
        
        let mut models = self.models.write().await;
        let versions = models.get_mut(model_name)
            .ok_or_else(|| anyhow::anyhow!("Model '{}' not found", model_name))?;
        
        for entry in versions.iter_mut() {
            if entry.deployment_status == DeploymentStatus::ABTesting {
                if entry.metadata.version.version == winner_version {
                    entry.deployment_status = DeploymentStatus::Production;
                } else {
                    entry.deployment_status = DeploymentStatus::Deprecated;
                }
                entry.ab_test_config = None;
            }
        }
        
        drop(models);
        
        // Set winner as active
        self.set_active_version(model_name, winner_version).await?;
        
        Ok(())
    }
    
    /// Deprecate a model version
    pub async fn deprecate_version(&self, model_name: &str, version: &str) -> Result<()> {
        info!("Deprecating model {} version {}", model_name, version);
        
        let mut models = self.models.write().await;
        let versions = models.get_mut(model_name)
            .ok_or_else(|| anyhow::anyhow!("Model '{}' not found", model_name))?;
        
        for entry in versions.iter_mut() {
            if entry.metadata.version.version == version {
                entry.deployment_status = DeploymentStatus::Deprecated;
                entry.metadata.deprecated_at = Some(Utc::now());
                return Ok(());
            }
        }
        
        Err(anyhow::anyhow!(
            "Version '{}' not found for model '{}'",
            version, model_name
        ))
    }
    
    /// Get model storage path
    pub fn get_model_path(&self, model_name: &str, version: &str) -> PathBuf {
        self.storage_path.join(format!("{}/{}.model", model_name, version))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[tokio::test]
    async fn test_model_registration() {
        let registry = ModelRegistry::new(env::temp_dir().join("test_models"));
        
        let metadata = ModelMetadata {
            version: ModelVersion::new("test_model", "v1.0"),
            description: "Test model".to_string(),
            model_type: ModelType::LstmPredictor,
            framework: "candle".to_string(),
            file_path: PathBuf::from("/tmp/model.bin"),
            file_size_bytes: 1024,
            checksum: "abc123".to_string(),
            hyperparameters: HashMap::new(),
            training_metrics: TrainingMetrics::default(),
            deployed_at: None,
            deprecated_at: None,
            tags: vec!["test".to_string()],
        };
        
        registry.register_model(metadata, DeploymentStatus::Staged).await.unwrap();
        
        let active = registry.get_active_version("test_model").await;
        assert_eq!(active, Some("v1.0".to_string()));
    }

    #[tokio::test]
    async fn test_version_switching() {
        let registry = ModelRegistry::new(env::temp_dir().join("test_models2"));
        
        // Register v1
        let metadata_v1 = ModelMetadata {
            version: ModelVersion::new("model", "v1"),
            description: "V1".to_string(),
            model_type: ModelType::LstmPredictor,
            framework: "candle".to_string(),
            file_path: PathBuf::from("/tmp/v1.bin"),
            file_size_bytes: 1024,
            checksum: "abc".to_string(),
            hyperparameters: HashMap::new(),
            training_metrics: TrainingMetrics::default(),
            deployed_at: None,
            deprecated_at: None,
            tags: vec![],
        };
        registry.register_model(metadata_v1, DeploymentStatus::Production).await.unwrap();
        
        // Register v2
        let metadata_v2 = ModelMetadata {
            version: ModelVersion::new("model", "v2"),
            description: "V2".to_string(),
            model_type: ModelType::LstmPredictor,
            framework: "candle".to_string(),
            file_path: PathBuf::from("/tmp/v2.bin"),
            file_size_bytes: 2048,
            checksum: "def".to_string(),
            hyperparameters: HashMap::new(),
            training_metrics: TrainingMetrics::default(),
            deployed_at: None,
            deprecated_at: None,
            tags: vec![],
        };
        registry.register_model(metadata_v2, DeploymentStatus::Staged).await.unwrap();
        
        // Switch to v2
        registry.set_active_version("model", "v2").await.unwrap();
        
        let active = registry.get_active_version("model").await;
        assert_eq!(active, Some("v2".to_string()));
    }
}

