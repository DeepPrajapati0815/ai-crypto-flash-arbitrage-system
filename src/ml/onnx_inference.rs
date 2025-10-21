//! ONNX Runtime inference for production ML models

use anyhow::{Result, Context};
use ort::{Environment, SessionBuilder, Value, GraphOptimizationLevel};
use ndarray::{Array2, ArrayD, CowArray, IxDyn};
use tracing::{info, debug};

/// ONNX model predictor for arbitrage opportunity scoring
pub struct ONNXPredictor { 
    session: ort::Session,
    input_name: String,
    output_name: String,
    input_size: usize,
}

impl ONNXPredictor {
    /// Create new ONNX predictor from model file
    pub fn new(model_path: &str) -> Result<Self> {
        info!("Loading ONNX model from: {}", model_path);
        
        // Create ONNX Runtime environment
        let environment = Environment::builder()
            .with_name("flash_arbitrage")
            .with_log_level(ort::LoggingLevel::Warning)
            .build()?
            .into_arc();
        
        // Create session with optimizations
        let session = SessionBuilder::new(&environment)?
            .with_optimization_level(GraphOptimizationLevel::Level3)?
            .with_intra_threads(4)?
            .with_inter_threads(2)?
            .with_model_from_file(model_path)
            .context("Failed to load ONNX model")?;
        
        // Get input/output names
        let input_name = session.inputs[0].name.clone();
        let output_name = session.outputs[0].name.clone();
        
        // Get input size
        let input_size = if session.inputs[0].dimensions.len() >= 2 {
            session.inputs[0].dimensions[1].unwrap_or(50) as usize
        } else {
            50 // Default to 50 features
        };
        
        info!("✅ ONNX model loaded successfully");
        info!("   Input:  {} (size: {})", input_name, input_size);
        info!("   Output: {}", output_name);
        
        Ok(Self {
            session,
            input_name,
            output_name,
            input_size,
        })
    }
    
    /// Predict probability for a single feature vector
    pub fn predict(&self, features: &[f32]) -> Result<f32> {
        // Validate input size
        if features.len() != self.input_size {
            return Err(anyhow::anyhow!(
                "Invalid input size: expected {}, got {}",
                self.input_size,
                features.len()
            ));
        }
        
        // Create input tensor [1, input_size] with dynamic dimensions
        let array = Array2::from_shape_vec((1, self.input_size), features.to_vec())?;
        let dyn_array: ArrayD<f32> = array.into_dyn();
        let cow_array: CowArray<f32, IxDyn> = CowArray::from(dyn_array.view());
        let input_tensor = Value::from_array(self.session.allocator(), &cow_array)?;
        
        // Run inference
        let outputs = self.session.run(vec![input_tensor])?;
        
        // Extract prediction
        let prediction = outputs[0]
            .try_extract::<f32>()?
            .view()
            .to_owned()
            [[0, 0]];
        
        debug!("ONNX prediction: {:.4}", prediction);
        
        Ok(prediction)
    }
    
    /// Predict probabilities for a batch of feature vectors
    pub fn predict_batch(&self, batch: &[Vec<f32>]) -> Result<Vec<f32>> {
        if batch.is_empty() {
            return Ok(Vec::new());
        }
        
        let batch_size = batch.len();
        
        // Flatten batch into single array
        let mut flattened = Vec::with_capacity(batch_size * self.input_size);
        for features in batch {
            if features.len() != self.input_size {
                return Err(anyhow::anyhow!(
                    "Invalid input size in batch: expected {}, got {}",
                    self.input_size,
                    features.len()
                ));
            }
            flattened.extend_from_slice(features);
        }
        
        // Create input tensor [batch_size, input_size] with dynamic dimensions
        let array = Array2::from_shape_vec((batch_size, self.input_size), flattened)?;
        let dyn_array: ArrayD<f32> = array.into_dyn();
        let cow_array: CowArray<f32, IxDyn> = CowArray::from(dyn_array.view());
        let input_tensor = Value::from_array(self.session.allocator(), &cow_array)?;
        
        // Run inference
        let outputs = self.session.run(vec![input_tensor])?;
        
        // Extract predictions
        let output_tensor = outputs[0].try_extract::<f32>()?;
        let output_view = output_tensor.view();
        let predictions: Vec<f32> = (0..batch_size)
            .map(|i| output_view[[i, 0]])
            .collect();
        
        debug!("ONNX batch prediction: {} samples", batch_size);
        
        Ok(predictions)
    }
    
    /// Get input feature size
    pub fn input_size(&self) -> usize {
        self.input_size
    }
    
    /// Get model metadata
    pub fn metadata(&self) -> ModelMetadata {
        ModelMetadata {
            input_name: self.input_name.clone(),
            output_name: self.output_name.clone(),
            input_size: self.input_size,
            num_inputs: self.session.inputs.len(),
            num_outputs: self.session.outputs.len(),
        }
    }
}

/// Model metadata
#[derive(Debug, Clone)]
pub struct ModelMetadata {
    pub input_name: String,
    pub output_name: String,
    pub input_size: usize,
    pub num_inputs: usize,
    pub num_outputs: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    #[ignore] // Requires actual ONNX model file
    fn test_onnx_predictor() {
        let predictor = ONNXPredictor::new("models/trading_model.onnx").unwrap();
        
        // Test prediction with dummy features
        let features = vec![0.5; 50];
        let prediction = predictor.predict(&features).unwrap();
        
        assert!(prediction >= 0.0 && prediction <= 1.0);
    }
    
    #[test]
    #[ignore]
    fn test_batch_prediction() {
        let predictor = ONNXPredictor::new("models/trading_model.onnx").unwrap();
        
        // Test batch
        let batch = vec![
            vec![0.5; 50],
            vec![0.7; 50],
            vec![0.3; 50],
        ];
        
        let predictions = predictor.predict_batch(&batch).unwrap();
        
        assert_eq!(predictions.len(), 3);
        for pred in predictions {
            assert!(pred >= 0.0 && pred <= 1.0);
        }
    }
    
    #[test]
    fn test_invalid_input_size() {
        // This test doesn't require an actual model
        // Just demonstrates the validation logic
        let features = vec![0.5; 30]; // Wrong size
        
        // Would fail with: "Invalid input size: expected 50, got 30"
        // assert!(predictor.predict(&features).is_err());
    }
}

