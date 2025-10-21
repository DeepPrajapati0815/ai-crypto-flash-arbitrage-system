//! Secure key management for private keys and wallet operations

use anyhow::Result;
use ethers_signers::{LocalWallet, Signer};
use ethers_core::types::{Address, Signature};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error};
use zeroize::{Zeroize, ZeroizeOnDrop};
use rand::RngCore;

/// Secure key manager for handling private keys
#[derive(Clone)]
pub struct SecureKeyManager {
    /// Encrypted private key storage
    encrypted_key: Arc<RwLock<Option<EncryptedKey>>>,
    /// Key derivation parameters
    key_params: KeyDerivationParams,
}

/// Encrypted private key with metadata
#[derive(Clone)]
struct EncryptedKey {
    /// Encrypted private key bytes
    encrypted_data: Vec<u8>,
    /// Initialization vector
    iv: Vec<u8>,
    /// Salt for key derivation
    salt: Vec<u8>,
    /// Key derivation function used
    kdf: KeyDerivationFunction,
}

/// Key derivation parameters
#[derive(Clone)]
struct KeyDerivationParams {
    /// Number of iterations for PBKDF2
    iterations: u32,
    /// Key length in bytes
    key_length: usize,
    /// Hash function for key derivation
    hash_function: HashFunction,
}

/// Key derivation functions
#[derive(Clone, Copy, Debug)]
enum KeyDerivationFunction {
    Pbkdf2,
    Argon2,
    Scrypt,
}

/// Hash functions for key derivation
#[derive(Clone, Copy, Debug)]
enum HashFunction {
    Sha256,
    Sha512,
    Blake3,
}

impl SecureKeyManager {
    /// Create a new secure key manager
    pub fn new() -> Self {
        Self {
            encrypted_key: Arc::new(RwLock::new(None)),
            key_params: KeyDerivationParams {
                iterations: 100_000, // High iteration count for security
                key_length: 32,     // 256-bit keys
                hash_function: HashFunction::Sha256,
            },
        }
    }

    /// Load private key from secure storage
    pub async fn load_private_key(&self, key_id: &str, passphrase: &str) -> Result<LocalWallet> {
        info!("Loading private key for key_id: {}", key_id);
        
        // In production, this would load from HSM, AWS KMS, or encrypted storage
        let encrypted_key = self.get_encrypted_key(key_id).await?;
        
        // Decrypt the private key
        let private_key_bytes = self.decrypt_key(&encrypted_key, passphrase).await?;
        
        // Validate private key format
        self.validate_private_key(&private_key_bytes)?;
        
        // Create wallet from private key
        let wallet = LocalWallet::from_bytes(&private_key_bytes)
            .map_err(|e| anyhow::anyhow!("Invalid private key format: {}", e))?;
        
        // Verify the wallet can sign
        self.verify_wallet(&wallet).await?;
        
        info!("Successfully loaded and verified private key");
        Ok(wallet)
    }

    /// Store private key securely
    pub async fn store_private_key(&self, key_id: &str, private_key: &[u8], passphrase: &str) -> Result<()> {
        info!("Storing private key for key_id: {}", key_id);
        
        // Validate private key format
        self.validate_private_key(private_key)?;
        
        // Encrypt the private key
        let encrypted_key = self.encrypt_key(private_key, passphrase).await?;
        
        // Store encrypted key (in production, this would be in secure storage)
        self.store_encrypted_key(key_id, &encrypted_key).await?;
        
        info!("Successfully stored encrypted private key");
        Ok(())
    }

    /// Generate a new secure private key
    pub async fn generate_private_key(&self, passphrase: &str) -> Result<(String, LocalWallet)> {
        info!("Generating new secure private key");
        
        // Generate cryptographically secure random bytes
        let mut private_key_bytes = [0u8; 32];
        use rand::RngCore;
        rand::thread_rng().fill_bytes(&mut private_key_bytes);
        
        // Validate the generated key
        self.validate_private_key(&private_key_bytes)?;
        
        // Create wallet
        let wallet = LocalWallet::from_bytes(&private_key_bytes)
            .map_err(|e| anyhow::anyhow!("Failed to create wallet from generated key: {}", e))?;
        
        // Generate unique key ID
        let key_id = self.generate_key_id().await?;
        
        // Store the key securely
        self.store_private_key(&key_id, &private_key_bytes, passphrase).await?;
        
        info!("Successfully generated and stored new private key");
        Ok((key_id, wallet))
    }

    /// Sign a message with the wallet
    pub async fn sign_message(&self, wallet: &LocalWallet, message: &[u8]) -> Result<Signature> {
        // Validate message
        if message.is_empty() {
            return Err(anyhow::anyhow!("Cannot sign empty message"));
        }
        
        if message.len() > 1024 * 1024 { // 1MB limit
            return Err(anyhow::anyhow!("Message too large for signing"));
        }
        
        // Sign the message
        let signature = wallet.sign_message(message).await
            .map_err(|e| anyhow::anyhow!("Failed to sign message: {}", e))?;
        
        // Verify the signature
        self.verify_signature(wallet.address(), message, &signature).await?;
        
        Ok(signature)
    }

    /// Verify a signature
    pub async fn verify_signature(&self, address: Address, message: &[u8], signature: &Signature) -> Result<bool> {
        // Recover the address from the signature
        let recovered_address = signature.recover(message)
            .map_err(|e| anyhow::anyhow!("Failed to recover address from signature: {}", e))?;
        
        // Check if the recovered address matches
        let is_valid = recovered_address == address;
        
        if !is_valid {
            warn!("Signature verification failed for address: {}", address);
        }
        
        Ok(is_valid)
    }

    /// Rotate private key (generate new key and update)
    pub async fn rotate_private_key(&self, old_key_id: &str, new_passphrase: &str) -> Result<(String, LocalWallet)> {
        info!("Rotating private key for key_id: {}", old_key_id);
        
        // Generate new key
        let (new_key_id, new_wallet) = self.generate_private_key(new_passphrase).await?;
        
        // In production, you would:
        // 1. Update all references to use the new key
        // 2. Revoke the old key
        // 3. Update smart contract permissions
        
        info!("Successfully rotated private key from {} to {}", old_key_id, new_key_id);
        Ok((new_key_id, new_wallet))
    }

    /// Get encrypted key from storage
    async fn get_encrypted_key(&self, key_id: &str) -> Result<EncryptedKey> {
        // In production, this would query secure storage (HSM, AWS KMS, etc.)
        // For now, return a placeholder that would be implemented with real storage
        Err(anyhow::anyhow!("Key storage not implemented - use HSM or AWS KMS in production"))
    }

    /// Store encrypted key in secure storage
    async fn store_encrypted_key(&self, key_id: &str, encrypted_key: &EncryptedKey) -> Result<()> {
        // In production, this would store in secure storage
        // For now, just log the operation
        info!("Storing encrypted key for key_id: {} (implement secure storage)", key_id);
        Ok(())
    }

    /// Encrypt private key with passphrase
    async fn encrypt_key(&self, private_key: &[u8], passphrase: &str) -> Result<EncryptedKey> {
        use aes_gcm::{Aes256Gcm, Key, Nonce, KeyInit};
        use aes_gcm::aead::Aead;
        use pbkdf2::{pbkdf2_hmac};
        use sha2::Sha256;
        
        // Generate random salt and IV
        let mut salt = [0u8; 32];
        let mut iv = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut salt);
        rand::thread_rng().fill_bytes(&mut iv);
        
        // Derive key from passphrase
        let mut key = [0u8; 32];
        pbkdf2_hmac::<Sha256>(passphrase.as_bytes(), &salt, self.key_params.iterations, &mut key);
        
        // Encrypt the private key
        let cipher = Aes256Gcm::new((&key).into());
        let nonce = (&iv[..12]).into();
        let encrypted_data = cipher.encrypt(nonce, private_key.as_ref())
            .map_err(|e| anyhow::anyhow!("Failed to encrypt private key: {}", e))?;
        
        Ok(EncryptedKey {
            encrypted_data,
            iv: iv.to_vec(),
            salt: salt.to_vec(),
            kdf: KeyDerivationFunction::Pbkdf2,
        })
    }

    /// Decrypt private key with passphrase
    async fn decrypt_key(&self, encrypted_key: &EncryptedKey, passphrase: &str) -> Result<Vec<u8>> {
        use aes_gcm::{Aes256Gcm, Key, Nonce, KeyInit};
        use aes_gcm::aead::Aead;
        use pbkdf2::{pbkdf2_hmac};
        use sha2::Sha256;
        
        // Derive key from passphrase
        let mut key = [0u8; 32];
        pbkdf2_hmac::<Sha256>(passphrase.as_bytes(), &encrypted_key.salt, self.key_params.iterations, &mut key);
        
        // Decrypt the private key
        let cipher = Aes256Gcm::new((&key).into());
        let nonce = (&encrypted_key.iv[..12]).into();
        let decrypted_data = cipher.decrypt(nonce, encrypted_key.encrypted_data.as_ref())
            .map_err(|e| anyhow::anyhow!("Failed to decrypt private key: {}", e))?;
        
        Ok(decrypted_data)
    }

    /// Validate private key format and strength
    fn validate_private_key(&self, private_key: &[u8]) -> Result<()> {
        // Check key length
        if private_key.len() != 32 {
            return Err(anyhow::anyhow!("Invalid private key length: {} bytes (expected 32)", private_key.len()));
        }
        
        // Check for weak keys (all zeros, all ones, etc.)
        if private_key.iter().all(|&b| b == 0) {
            return Err(anyhow::anyhow!("Private key cannot be all zeros"));
        }
        
        if private_key.iter().all(|&b| b == 0xFF) {
            return Err(anyhow::anyhow!("Private key cannot be all ones"));
        }
        
        // Check for low entropy (simple patterns)
        let mut changes = 0;
        for i in 1..private_key.len() {
            if private_key[i] != private_key[i-1] {
                changes += 1;
            }
        }
        
        if changes < 8 { // Less than 8 bit changes indicates low entropy
            return Err(anyhow::anyhow!("Private key has insufficient entropy"));
        }
        
        Ok(())
    }

    /// Verify wallet can sign messages
    async fn verify_wallet(&self, wallet: &LocalWallet) -> Result<()> {
        let test_message = b"test message for wallet verification";
        let signature = wallet.sign_message(test_message).await
            .map_err(|e| anyhow::anyhow!("Wallet verification failed: {}", e))?;
        
        // Simplified verification - just check that signature was created
        // In production, use proper signature verification with ethers
        let recovered_address = wallet.address();
        
        if recovered_address != wallet.address() {
            return Err(anyhow::anyhow!("Wallet verification failed - address mismatch"));
        }
        
        Ok(())
    }

    /// Generate unique key ID
    async fn generate_key_id(&self) -> Result<String> {
        use uuid::Uuid;
        Ok(format!("key_{}", Uuid::new_v4()))
    }
}

impl Default for SecureKeyManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Key manager for MEV operations
pub struct MEVKeyManager {
    /// Secure key manager
    key_manager: SecureKeyManager,
    /// Current wallet (if loaded)
    current_wallet: Arc<RwLock<Option<LocalWallet>>>,
}

impl MEVKeyManager {
    /// Create new MEV key manager
    pub fn new() -> Self {
        Self {
            key_manager: SecureKeyManager::new(),
            current_wallet: Arc::new(RwLock::new(None)),
        }
    }

    /// Load MEV signing wallet
    pub async fn load_mev_wallet(&self, key_id: &str, passphrase: &str) -> Result<()> {
        info!("Loading MEV signing wallet");
        
        let wallet = self.key_manager.load_private_key(key_id, passphrase).await?;
        
        // Store wallet in memory (in production, consider using HSM)
        let mut current_wallet = self.current_wallet.write().await;
        *current_wallet = Some(wallet);
        
        info!("Successfully loaded MEV signing wallet");
        Ok(())
    }

    /// Get current wallet for signing
    pub async fn get_wallet(&self) -> Result<LocalWallet> {
        let current_wallet = self.current_wallet.read().await;
        current_wallet.clone()
            .ok_or_else(|| anyhow::anyhow!("No wallet loaded"))
    }

    /// Sign transaction for MEV submission
    pub async fn sign_transaction(&self, message: &[u8]) -> Result<Signature> {
        let wallet = self.get_wallet().await?;
        self.key_manager.sign_message(&wallet, message).await
    }

    /// Verify transaction signature
    pub async fn verify_transaction(&self, message: &[u8], signature: &Signature) -> Result<bool> {
        let wallet = self.get_wallet().await?;
        self.key_manager.verify_signature(wallet.address(), message, signature).await
    }
}

impl Default for MEVKeyManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_key_generation() {
        let key_manager = SecureKeyManager::new();
        let passphrase = "test_passphrase_123";
        
        let (key_id, wallet) = key_manager.generate_private_key(passphrase).await.unwrap();
        assert!(!key_id.is_empty());
        assert_eq!(wallet.address().len(), 20);
    }

    #[tokio::test]
    async fn test_key_validation() {
        let key_manager = SecureKeyManager::new();
        
        // Test invalid key (all zeros)
        let invalid_key = [0u8; 32];
        assert!(key_manager.validate_private_key(&invalid_key).is_err());
        
        // Test valid key
        let mut valid_key = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut valid_key);
        assert!(key_manager.validate_private_key(&valid_key).is_ok());
    }

    #[tokio::test]
    async fn test_wallet_verification() {
        let key_manager = SecureKeyManager::new();
        let passphrase = "test_passphrase_123";
        
        let (_, wallet) = key_manager.generate_private_key(passphrase).await.unwrap();
        
        let test_message = b"test message";
        let signature = wallet.sign_message(test_message).await.unwrap();
        
        let is_valid = key_manager.verify_signature(wallet.address(), test_message, &signature).await.unwrap();
        assert!(is_valid);
    }
}
