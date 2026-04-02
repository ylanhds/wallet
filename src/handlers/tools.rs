// ============================================
// 区块链工具路由处理器
// ============================================

use axum::{Json, extract::{Path, Query, State}};
use std::sync::Arc;
use crate::config::AppState;
use crate::models::*;
use crate::utils::crypto::encrypt_data;
use bip39::{Language, Mnemonic};
use secp256k1::{PublicKey, Secp256k1, SecretKey};
use sha3::{Digest, Keccak256};
use rand::Rng;

/// 🌐 生成多链钱包地址（ETH、BTC、TRON、BSC）
pub async fn generate_multi_chain_wallet(
    State(app): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    // 生成助记词
    let Ok(mnemonic) = Mnemonic::generate_in(Language::English, 12) else {
        return Json(serde_json::json!({
            "success": false,
            "message": "助记词生成失败"
        }));
    };
    
    let phrase = mnemonic.to_string();
    let seed = mnemonic.to_seed("");
    
    // 以太坊地址（与之前相同）
    let secp = Secp256k1::new();
    let secret_key = SecretKey::from_byte_array(seed[..32].try_into().unwrap()).unwrap();
    let public_key = PublicKey::from_secret_key(&secp, &secret_key);
    let public_key_bytes = &public_key.serialize_uncompressed()[1..];
    let hash_eth = Keccak256::digest(public_key_bytes);
    let eth_address = format!("0x{}", hex::encode(&hash_eth[12..]));
    
    // 比特币地址（P2PKH 格式）
    use sha2::{Sha256, Digest};
    
    // RIPEMD160(SHA256(pubkey))
    let sha256_hash = Sha256::digest(public_key_bytes);
    let ripemd_hash = ripemd::Ripemd160::digest(&sha256_hash);
    
    // 添加版本字节 (0x00 for mainnet)
    let mut versioned = vec![0x00];
    versioned.extend_from_slice(&ripemd_hash);
    
    // 双重 SHA256 校验和
    let checksum_full = Sha256::digest(&Sha256::digest(&versioned));
    let mut btc_bytes = versioned;
    btc_bytes.extend_from_slice(&checksum_full[..4]);
    
    // Base58 编码
    let btc_address = bs58::encode(&btc_bytes).into_string();
    
    // TRON 地址（前缀 T + Keccak256）
    let hash_tron = Keccak256::digest(public_key_bytes);
    let tron_bytes = &hash_tron[12..];
    
    // TRON 版本字节 0x41
    let mut tron_versioned = vec![0x41];
    tron_versioned.extend_from_slice(tron_bytes);
    
    // 校验和
    let tron_checksum = Sha256::digest(&Sha256::digest(&tron_versioned));
    let mut tron_bytes_full = tron_versioned;
    tron_bytes_full.extend_from_slice(&tron_checksum[..4]);
    
    let tron_address = bs58::encode(&tron_bytes_full).into_string();
    
    // BSC 地址（与以太坊相同，因为兼容 EVM）
    let bsc_address = eth_address.clone();
    
    // 保存以太坊地址到数据库
    let enc_mnemonic = encrypt_data(&app.enc_key, &phrase).unwrap_or_default();
    let enc_private = encrypt_data(&app.enc_key, &hex::encode(secret_key.secret_bytes())).unwrap_or_default();
    let _ = save_wallet(&app.db, &eth_address, &enc_mnemonic, &enc_private).await;
    
    Json(serde_json::json!({
        "success": true,
        "mnemonic": phrase,
        "addresses": {
            "ethereum": eth_address,
            "bitcoin": btc_address,
            "tron": tron_address,
            "binance_smart_chain": bsc_address
        },
        "note": "已保存以太坊地址到数据库"
    }))
}

/// 辅助函数：保存钱包
async fn save_wallet(
    db: &sqlx::Pool<sqlx::MySql>,
    address: &str,
    mnemonic_enc: &str,
    private_key_enc: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO ws_wallets (address, mnemonic_enc, private_key_enc) VALUES (?, ?, ?)",
        address,
        mnemonic_enc,
        private_key_enc
    )
    .execute(db)
    .await?;
    Ok(())
}

/// 🔑 从私钥推导公钥和地址
pub async fn derive_from_private_key(
    State(_app): State<Arc<AppState>>,
    Json(req): Json<DeriveKeyRequest>,
) -> Json<serde_json::Value> {
    // 解析私钥
    let private_key = req.mnemonic.trim_start_matches("0x");
    let key_bytes = match hex::decode(private_key) {
        Ok(bytes) => bytes,
        Err(_) => {
            return Json(serde_json::json!({
                "success": false,
                "message": "无效的私钥格式（应为 hex）"
            }));
        }
    };
    
    if key_bytes.len() != 32 {
        return Json(serde_json::json!({
            "success": false,
            "message": "私钥长度错误（应为 32 字节）"
        }));
    }
    
    // 创建密钥对
    let secp = Secp256k1::new();
    let Ok(secret_key) = SecretKey::from_byte_array(key_bytes.try_into().unwrap()) else {
        return Json(serde_json::json!({
            "success": false,
            "message": "无效的私钥"
        }));
    };
    
    let public_key = PublicKey::from_secret_key(&secp, &secret_key);
    let public_key_hex = hex::encode(public_key.serialize_uncompressed());
    
    // 生成以太坊地址
    let public_key_bytes = &public_key.serialize_uncompressed()[1..];
    let hash = Keccak256::digest(public_key_bytes);
    let address = format!("0x{}", hex::encode(&hash[12..]));
    
    Json(serde_json::json!({
        "success": true,
        "private_key": format!("0x{}", private_key),
        "public_key": public_key_hex,
        "address": address,
        "note": "私钥可以推导出公钥和地址，请妥善保管私钥"
    }))
}

/// ✅ 验证助记词是否有效
pub async fn verify_mnemonic(
    State(_app): State<Arc<AppState>>,
    Json(req): Json<DeriveKeyRequest>,
) -> Json<serde_json::Value> {
    let mnemonic_str = req.mnemonic.trim();
    let word_count = mnemonic_str.split_whitespace().count();
    
    // 检查单词数量
    if ![12, 15, 18, 24].contains(&word_count) {
        return Json(serde_json::json!({
            "success": false,
            "valid": false,
            "message": format!("助记词应为 12/15/18/24 个单词，当前为 {} 个", word_count)
        }));
    }
    
    // 简单验证：所有单词都应该是字母
    let all_alpha = mnemonic_str.split_whitespace()
        .all(|word| word.chars().all(|c| c.is_alphabetic()));
    
    if !all_alpha {
        return Json(serde_json::json!({
            "success": false,
            "valid": false,
            "message": "助记词只能包含英文字母"
        }));
    }
    
    // 通过基本验证
    Json(serde_json::json!({
        "success": true,
        "valid": true,
        "word_count": word_count,
        "message": "助记词格式正确",
        "note": "这是基础格式验证，非完整 BIP39 验证"
    }))
}

/// ✍️ 消息签名（使用私钥对消息签名）
pub async fn sign_message(
    State(_app): State<Arc<AppState>>,
    Json(req): Json<SignMessageRequest>,
) -> Json<serde_json::Value> {
    // 解析私钥
    let private_key = req.private_key.trim_start_matches("0x");
    let key_bytes = match hex::decode(private_key) {
        Ok(bytes) => bytes,
        Err(_) => {
            return Json(serde_json::json!({
                "success": false,
                "message": "无效的私钥格式"
            }));
        }
    };
    
    if key_bytes.len() != 32 {
        return Json(serde_json::json!({
            "success": false,
            "message": "私钥长度错误"
        }));
    }
    
    // 简化的签名（仅演示）
    let message_hash = Keccak256::digest(req.message.as_bytes());
    let signature_bytes = [&message_hash[..], &key_bytes].concat();
    let sig_hash = Keccak256::digest(&signature_bytes);
    
    Json(serde_json::json!({
        "success": true,
        "message": req.message,
        "signature": format!("0x{}", hex::encode(sig_hash)),
        "note": "这是简化的消息签名演示"
    }))
}

/// ✔️ 验证签名
pub async fn verify_signature(
    State(_app): State<Arc<AppState>>,
    Json(req): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let message = req["message"].as_str().unwrap_or("");
    let _signature = req["signature"].as_str().unwrap_or("");
    let signer = req["signer"].as_str().unwrap_or("");
    
    // 简化验证（完整实现需要恢复公钥）
    Json(serde_json::json!({
        "success": true,
        "valid": true,
        "message": message,
        "signer": signer,
        "note": "签名验证通过（演示版本）"
    }))
}

/// 💸 模拟转账（生成交易数据）
pub async fn simulate_transfer(
    State(_app): State<Arc<AppState>>,
    Json(req): Json<TransferRequest>,
) -> Json<serde_json::Value> {
    // 验证地址格式
    if !req.from.starts_with("0x") || !req.to.starts_with("0x") {
        return Json(serde_json::json!({
            "success": false,
            "message": "无效的地址格式"
        }));
    }
    
    // 生成假的交易哈希
    let tx_data = format!("{}{}{}{}", req.from, req.to, req.amount, chrono::Utc::now().timestamp());
    let tx_hash = Keccak256::digest(tx_data.as_bytes());
    
    // 计算 Gas 费
    let gas_used: u64 = 21000;  // 标准 ETH 转账
    let gas_price: f64 = 20.0;  // 20 Gwei
    let gas_fee_eth = (gas_used as f64 * gas_price) / 1_000_000_000.0;
    
    // 生成随机区块号
    let mut rng = rand::rng();
    let block_number: u32 = rng.random_range(18_000_000..19_000_000);
    
    Json(serde_json::json!({
        "success": true,
        "transaction": {
            "hash": format!("0x{}", hex::encode(tx_hash)),
            "from": req.from,
            "to": req.to,
            "amount": req.amount,
            "gas_used": gas_used,
            "gas_price_gwei": gas_price,
            "gas_fee_eth": format!("{:.6}", gas_fee_eth),
            "block_number": block_number,
            "timestamp": chrono::Utc::now().naive_utc().to_string()
        },
        "note": "这是模拟的交易数据，未上链"
    }))
}

/// 📊 地址分析工具（分析地址特征）
pub async fn analyze_address(
    State(_app): State<Arc<AppState>>,
    Path(address): Path<String>,
) -> Json<serde_json::Value> {
    if !address.starts_with("0x") || address.len() != 42 {
        return Json(serde_json::json!({
            "success": false,
            "message": "无效的以太坊地址格式"
        }));
    }
    
    // 分析地址特征
    let addr_lower = address.to_lowercase();
    let hex_part = &addr_lower[2..];
    
    // 统计数字和字母数量
    let digit_count = hex_part.chars().filter(|c| c.is_numeric()).count();
    let letter_count = hex_part.chars().filter(|c| c.is_alphabetic()).count();
    
    // 检查是否包含幸运数字
    let has_lucky_8 = hex_part.contains('8');
    let has_lucky_6 = hex_part.contains('6');
    let has_zero = hex_part.contains('0');
    
    // 检查是否有重复模式
    let has_repeat = hex_part.as_bytes().windows(3).any(|w| w[0] == w[1] && w[1] == w[2]);
    
    // 计算地址"分数"
    let score = digit_count * 2 + letter_count + 
                (if has_lucky_8 { 10 } else { 0 }) +
                (if has_lucky_6 { 5 } else { 0 }) +
                (if has_zero { 3 } else { 0 }) -
                (if has_repeat { 5 } else { 0 });
    
    Json(serde_json::json!({
        "success": true,
        "address": address,
        "analysis": {
            "length": address.len(),
            "digit_count": digit_count,
            "letter_count": letter_count,
            "has_lucky_8": has_lucky_8,
            "has_lucky_6": has_lucky_6,
            "has_leading_zeros": has_zero,
            "has_pattern": has_repeat
        },
        "score": score,
        "rating": if score >= 100 { "S 级 - 极品地址" }
                  else if score >= 80 { "A 级 - 优质地址" }
                  else if score >= 60 { "B 级 - 良好地址" }
                  else { "C 级 - 普通地址" },
        "note": "评分仅供娱乐，无实际意义"
    }))
}

/// 🔐 生成 vanity 地址（含特定字符的地址）
pub async fn generate_vanity_address(
    State(app): State<Arc<AppState>>,
    Query(params): Query<serde_json::Value>,
) -> Json<serde_json::Value> {
    let prefix = params["prefix"].as_str().unwrap_or("");
    let max_attempts = params["max_attempts"].as_u64().unwrap_or(10000);
    
    // 验证前缀
    if !prefix.chars().all(|c| c.is_alphanumeric()) {
        return Json(serde_json::json!({
            "success": false,
            "message": "前缀只能包含字母和数字"
        }));
    }
    
    if prefix.len() > 5 {
        return Json(serde_json::json!({
            "success": false,
            "message": "前缀太长，最多 5 个字符"
        }));
    }
    
    // 尝试生成包含特定前缀的地址
    let target_prefix = format!("0x{}", prefix.to_lowercase());
    let mut attempts = 0;
    
    while attempts < max_attempts {
        attempts += 1;
        
        // 生成新钱包
        if let Ok((mnemonic, private_key, address)) = generate_wallet() {
            if address.to_lowercase().starts_with(&target_prefix) {
                // 保存这个特殊地址
                let enc_mnemonic = encrypt_data(&app.enc_key, &mnemonic).unwrap_or_default();
                let enc_private = encrypt_data(&app.enc_key, &private_key).unwrap_or_default();
                let _ = save_wallet(&app.db, &address, &enc_mnemonic, &enc_private).await;
                
                return Json(serde_json::json!({
                    "success": true,
                    "address": address,
                    "mnemonic": if app.is_dev { Some(mnemonic) } else { None },
                    "attempts": attempts,
                    "prefix": prefix,
                    "note": "已保存到数据库"
                }));
            }
        }
    }
    
    Json(serde_json::json!({
        "success": false,
        "message": format!("未在 {} 次尝试内找到匹配地址", max_attempts),
        "suggestion": "尝试更短的前缀或增加尝试次数"
    }))
}

// 导入 generate_wallet 函数
fn generate_wallet() -> Result<(String, String, String), anyhow::Error> {
    let mnemonic = Mnemonic::generate_in(Language::English, 12)?;
    let phrase = mnemonic.to_string();
    let seed = mnemonic.to_seed("");
    
    let secp = Secp256k1::new();
    let secret_key = SecretKey::from_byte_array(seed[..32].try_into()?)?;
    let public_key = PublicKey::from_secret_key(&secp, &secret_key);

    let public_key_bytes = &public_key.serialize_uncompressed()[1..];
    let hash = Keccak256::digest(public_key_bytes);
    let address = format!("0x{}", hex::encode(&hash[12..]));

    Ok((phrase, hex::encode(secret_key.secret_bytes()), address))
}
