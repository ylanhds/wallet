// ============================================
// 加密工具模块
// ============================================

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::{Engine as _, engine::general_purpose};
use rand::RngCore;

/// 加密数据（AES-256-GCM）
pub fn encrypt_data(key: &[u8; 32], plaintext: &str) -> Result<String, anyhow::Error> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    
    // 生成随机 nonce
    let mut nonce_bytes = [0u8; 12];
    rand::rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    
    // 加密
    let ciphertext = cipher.encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| anyhow::anyhow!("加密失败：{}", e))?;
    
    // 返回 "nonce:ciphertext" 的 base64 编码格式
    Ok(format!(
        "{}:{}",
        general_purpose::STANDARD.encode(nonce_bytes),
        general_purpose::STANDARD.encode(ciphertext)
    ))
}

/// 解密数据（AES-256-GCM）
pub fn decrypt_data(key: &[u8; 32], combined: &str) -> Option<String> {
    let parts: Vec<&str> = combined.split(':').collect();
    if parts.len() != 2 {
        return None;
    }
    
    let nonce_bytes = general_purpose::STANDARD.decode(parts[0]).ok()?;
    let ciphertext_bytes = general_purpose::STANDARD.decode(parts[1]).ok()?;
    
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Nonce::from_slice(&nonce_bytes);
    
    let plaintext = cipher.decrypt(nonce, ciphertext_bytes.as_ref()).ok()?;
    Some(String::from_utf8_lossy(&plaintext).to_string())
}
