// ============================================
// 公共路由处理器（健康检查、统计等）
// ============================================

use axum::{Json, extract::{Query, State}};
use sqlx::Row;
use std::sync::Arc;
use crate::config::AppState;
use crate::models::*;
use sha3::{Digest, Keccak256};

/// 健康检查端点
pub async fn health_check(State(app): State<Arc<AppState>>) -> Json<HealthResponse> {
    // 简单测试数据库连接
    let db_status = sqlx::query("SELECT 1")
        .fetch_optional(&app.db)
        .await
        .map_or("disconnected", |_| "connected");
    
    Json(HealthResponse {
        status: "healthy".to_string(),
        database: db_status.to_string(),
        timestamp: chrono::Utc::now().naive_utc().to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// 获取统计信息
pub async fn get_stats(State(app): State<Arc<AppState>>) -> Json<StatsResponse> {
    // 获取总数
    let total = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM ws_wallets")
        .fetch_one(&app.db)
        .await
        .unwrap_or(0) as usize;

    // 获取今天的数量
    let today = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM ws_wallets WHERE DATE(created_at) = CURDATE()"
    )
    .fetch_one(&app.db)
    .await
    .unwrap_or(0) as usize;

    // 获取本周的数量
    let this_week = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM ws_wallets WHERE YEARWEEK(created_at) = YEARWEEK(NOW())"
    )
    .fetch_one(&app.db)
    .await
    .unwrap_or(0) as usize;

    // 获取第一个和最后一个钱包的时间
    let first = sqlx::query_scalar::<_, chrono::DateTime<chrono::Utc>>(
        "SELECT created_at FROM ws_wallets ORDER BY id ASC LIMIT 1"
    )
    .fetch_optional(&app.db)
    .await
    .ok()
    .flatten();

    let last = sqlx::query_scalar::<_, chrono::DateTime<chrono::Utc>>(
        "SELECT created_at FROM ws_wallets ORDER BY id DESC LIMIT 1"
    )
    .fetch_optional(&app.db)
    .await
    .ok()
    .flatten();

    // 计算平均每天创建数量
    let avg_per_day = if let (Some(first_dt), Some(last_dt)) = (&first, &last) {
        let days = (last_dt.timestamp() - first_dt.timestamp()) / 86400;
        if days > 0 { total as f64 / days as f64 } else { total as f64 }
    } else {
        0.0
    };

    Json(StatsResponse {
        total_wallets: total,
        created_today: today,
        created_this_week: this_week,
        avg_per_day,
        first_wallet_date: first.map(|dt| dt.naive_utc().to_string()),
        last_wallet_date: last.map(|dt| dt.naive_utc().to_string()),
    })
}

/// 获取最近活动
pub async fn get_recent_activity(
    State(app): State<Arc<AppState>>,
    Query(params): Query<serde_json::Value>,
) -> Json<ActivityResponse> {
    let limit = params["limit"].as_u64().unwrap_or(10) as usize;
    
    let records = sqlx::query(
        "SELECT address, created_at FROM ws_wallets ORDER BY id DESC LIMIT ?"
    )
    .bind(limit as i64)
    .fetch_all(&app.db)
    .await
    .unwrap_or_default();

    let recent: Vec<RecentWallet> = records
        .iter()
        .map(|row| {
            let address: Vec<u8> = row.try_get("address").unwrap();
            let created_at = row
                .try_get::<chrono::DateTime<chrono::Utc>, _>("created_at")
                .unwrap();
            
            RecentWallet {
                address: String::from_utf8(address).unwrap(),
                created_at: created_at.naive_utc().to_string(),
            }
        })
        .collect();

    Json(ActivityResponse {
        recent,
        limit,
    })
}

/// 验证以太坊地址
pub async fn validate_address(Json(req): Json<ValidateAddressRequest>) -> Json<ValidateResponse> {
    // 简单的格式验证：0x 开头 + 40 个十六进制字符
    let is_valid = req.address.starts_with("0x") && 
                   req.address.len() == 42 &&
                   req.address[2..].chars().all(|c| c.is_ascii_hexdigit());

    Json(ValidateResponse {
        valid: is_valid,
        message: if is_valid {
            "有效的以太坊地址格式".to_string()
        } else {
            "无效的地址格式。应为 0x 开头 + 40 个十六进制字符".to_string()
        },
    })
}

/// 生成随机助记词（不保存）
pub async fn generate_mnemonic_only() -> Json<MnemonicOnlyResponse> {
    use bip39::{Language, Mnemonic};
    
    match Mnemonic::generate_in(Language::English, 12) {
        Ok(mnemonic) => Json(MnemonicOnlyResponse {
            mnemonic: mnemonic.to_string(),
            message: "已生成随机助记词（未保存到数据库）".into(),
        }),
        Err(e) => Json(MnemonicOnlyResponse {
            mnemonic: String::new(),
            message: format!("生成失败：{}", e),
        }),
    }
}

/// 从私钥生成地址（不保存）
pub async fn address_from_private_key(Json(req): Json<PrivateKeyRequest>) -> Json<WalletResponse> {
    use secp256k1::{Secp256k1, SecretKey};
    
    // 移除 0x 前缀（如果有）
    let key_str = req.private_key.trim_start_matches("0x");
    
    // 尝试解析私钥
    match hex::decode(key_str) {
        Ok(key_bytes) => {
            if key_bytes.len() != 32 {
                return Json(WalletResponse {
                    address: String::new(),
                    message: "私钥长度应为 32 字节（64 个十六进制字符）".into(),
                    mnemonic: None,
                    private_key: None,
                });
            }

            match SecretKey::from_byte_array(key_bytes.try_into().unwrap()) {
                Ok(secret_key) => {
                    let secp = Secp256k1::new();
                    let public_key = secp256k1::PublicKey::from_secret_key(&secp, &secret_key);
                    let public_key_bytes = &public_key.serialize_uncompressed()[1..];
                    let hash = Keccak256::digest(public_key_bytes);
                    let address = format!("0x{}", hex::encode(&hash[12..]));

                    Json(WalletResponse {
                        address,
                        message: "成功从私钥生成地址（未保存）".into(),
                        mnemonic: None,
                        private_key: Some(req.private_key),
                    })
                }
                Err(_) => Json(WalletResponse {
                    address: String::new(),
                    message: "无效的私钥".into(),
                    mnemonic: None,
                    private_key: None,
                }),
            }
        }
        Err(_) => Json(WalletResponse {
            address: String::new(),
            message: "私钥格式无效（应为十六进制）".into(),
            mnemonic: None,
            private_key: None,
        }),
    }
}
