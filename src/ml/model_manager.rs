//! Model management with hot-reload and versioning

use crate::ml::onnx_inference::ONNXPredictor;
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error};
use serde::{Deserialize, Serialize};

/// Manages ONNX models with hot-reload capability
pub struct ModelManager {
    current_model: Arc<RwLock<ONNXPredictor>>,
    model_path: PathBuf,
    model_version: Arc<RwLock<String>>,
    model_info: Arc<RwLock<ModelInfo>>,
}

/// Model information from metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub model_version: String,
    pub created_at: String,
    pub accuracy: f64,
    pub avg_latency_ms: f64,
    pub input_features: usize,
}

impl Default for ModelInfo {
    fn default() -> Self {
        Self {
            model_version: "unknown".to_string(),
            created_at: "unknown".to_string(),
            accuracy: 0.0,
            avg_latency_ms: 0.0,
            input_features: 50,
        }
    }
}

impl ModelManager {
    /// Create new model manager and load initial model
    pub async fn new(model_path: impl AsRef<Path>) -> Result<Self> {
        let model_path = model_path.as_ref().to_path_buf();
        
        info!("Initializing model manager with: {:?}", model_path);
        
        // Load initial model
        let predictor = ONNXPredictor::new(model_path.to_str().unwrap())?;
        
        // Load metadata if available
        let model_info = Self::load_metadata(&model_path).unwrap_or_default();
        let version = model_info.model_version.clone();
        
        info!("✅ Model manager initialized (version: {})", version);
        
        Ok(Self {
            current_model: Arc::new(RwLock::new(predictor)),
            model_path,
            model_version: Arc::new(RwLock::new(version)),
            model_info: Arc::new(RwLock::new(model_info)),
        })
    }
    
    /// Predict probability for a single feature vector
    pub async fn predict(&self, features: &[f32]) -> Result<f32> {
        let model = self.current_model.read().await;
        model.predict(features)
    }
    
    /// Predict probabilities for a batch
    pub async fn predict_batch(&self, batch: &[Vec<f32>]) -> Result<Vec<f32>> {
        let model = self.current_model.read().await;
        model.predict_batch(batch)
    }
    
    /// Hot-reload model from a new path (zero-downtime update)
    pub async fn hot_reload(&self, new_model_path: impl AsRef<Path>) -> Result<()> {
        let new_model_path = new_model_path.as_ref();
        
        info!("Hot-reloading model from: {:?}", new_model_path);
        
        // Load new model
        let new_predictor = ONNXPredictor::new(new_model_path.to_str().unwrap())?;
        
        // Load new metadata
        let new_info = Self::load_metadata(new_model_path).unwrap_or_default();
        let new_version = new_info.model_version.clone();
        
        // Atomic swap (this is the "hot reload" - no downtime)
        {
            let mut model = self.current_model.write().await;
            *model = new_predictor;
        }
        
        // Update version and info
        {
            let mut version = self.model_version.write().await;
            *version = new_version.clone();
        }
        {
            let mut info = self.model_info.write().await;
            *info = new_info;
        }
        
        info!("✅ Model hot-reloaded successfully (new version: {})", new_version);
        
        Ok(())
    }
    
    /// Reload from the original path (useful for updates)
    pub async fn reload(&self) -> Result<()> {
        self.hot_reload(&self.model_path).await
    }
    
    /// Get current model version
    pub async fn version(&self) -> String {
        self.model_version.read().await.clone()
    }
    
    /// Get current model info
    pub async fn info(&self) -> ModelInfo {
        self.model_info.read().await.clone()
    }
    
    /// Load model metadata from JSON file
    fn load_metadata(model_path: &Path) -> Result<ModelInfo> {
        // Try to find metadata file in same directory
        let metadata_path = model_path
            .parent()
            .unwrap()
            .join("model_metadata.json");
        
        if !metadata_path.exists() {
            warn!("Metadata file not found: {:?}", metadata_path);
            return Ok(ModelInfo::default());
        }
        
        let content = std::fs::read_to_string(&metadata_path)?;
        let info: ModelInfo = serde_json::from_str(&content)?;
        
        Ok(info)
    }
    
    /// Get statistics for monitoring
    pub async fn get_stats(&self) -> ModelStats {
        let version = self.version().await;
        let info = self.info().await;
        
        ModelStats {
            model_version: version,
            accuracy: info.accuracy,
            avg_latency_ms: info.avg_latency_ms,
            input_features: info.input_features,
        }
    }
}

/// Model statistics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelStats {
    pub model_version: String,
    pub accuracy: f64,
    pub avg_latency_ms: f64,
    pub input_features: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    #[ignore] // Requires actual model file
    async fn test_model_manager() {
        let manager = ModelManager::new("models/trading_model.onnx").await.unwrap();
        
        // Test prediction
        let features = vec![0.5; 50];
        let prediction = manager.predict(&features).await.unwrap();
        
        assert!(prediction >= 0.0 && prediction <= 1.0);
    }
    
    #[tokio::test]
    #[ignore]
    async fn test_hot_reload() {
        let manager = ModelManager::new("models/trading_model.onnx").await.unwrap();
        
        let version_before = manager.version().await;
        
        // Simulate hot reload (same file for test)
        manager.reload().await.unwrap();
        
        let version_after = manager.version().await;
        
        // Version should remain same (same model)
        assert_eq!(version_before, version_after);
    }
    
    #[tokio::test]
    #[ignore]
    async fn test_batch_prediction() {
        let manager = ModelManager::new("models/trading_model.onnx").await.unwrap();
        
        let batch = vec![
            vec![0.5; 50],
            vec![0.7; 50],
        ];
        
        let predictions = manager.predict_batch(&batch).await.unwrap();
        
        assert_eq!(predictions.len(), 2);
    }
}

