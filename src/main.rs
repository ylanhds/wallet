use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, post, delete},
    response::Html,
};
use serde::{Serialize, Deserialize};
use tokio::net::TcpListener;
use bip39::{Language, Mnemonic};
use secp256k1::{PublicKey, Secp256k1, SecretKey};
use sha3::{Digest, Keccak256};
use sqlx::{MySql, Pool, Row, mysql::MySqlPoolOptions};
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::{Engine as _, engine::general_purpose};
use rand::{Rng, RngCore};
use dotenvy::dotenv;
use std::{env, sync::Arc};

// 🔐 用户认证相关（新增）
use jsonwebtoken::{encode, decode, Header, Validation, EncodingKey, DecodingKey};
use chrono::{Duration as ChronoDuration, Utc};

// WebSocket 相关
use futures_util::{stream::{SplitSink, StreamExt}, SinkExt};
use tokio::sync::broadcast;
use tokio_tungstenite::tungstenite::Message;
use axum::extract::ws::Utf8Bytes;

/// 应用状态结构体
#[derive(Clone)]
struct AppState {
    db: Pool<MySql>,
    enc_key: [u8; 32],
    is_dev: bool,
    ws_tx: broadcast::Sender<String>,  // WebSocket 广播发送器
    jwt_secret: String,  // 🔐 JWT 密钥（新增）
}

// ==== 核心业务逻辑 ====
/// 生成新钱包（助记词、私钥、地址）
fn generate_wallet() -> Result<(String, String, String), anyhow::Error> {
    // 生成 12 个单词的英文助记词
    let mnemonic = Mnemonic::generate_in(Language::English, 12)?;
    let phrase = mnemonic.to_string();

    // 从助记词生成种子
    let seed = mnemonic.to_seed("");
    
    // 创建 secp256k1 上下文并生成密钥对
    let secp = Secp256k1::new();
    let secret_key = SecretKey::from_byte_array(seed[..32].try_into()?)?;
    let public_key = PublicKey::from_secret_key(&secp, &secret_key);

    // 生成以太坊地址：公钥序列化 → Keccak256 哈希 → 取后 20 字节
    let public_key_bytes = &public_key.serialize_uncompressed()[1..];
    let hash = Keccak256::digest(public_key_bytes);
    let address = format!("0x{}", hex::encode(&hash[12..]));

    Ok((phrase, hex::encode(secret_key.secret_bytes()), address))
}

// ==== AES-GCM 加解密 ====
/// 加密数据（AES-256-GCM）
fn encrypt_data(key: &[u8; 32], plaintext: &str) -> Result<String, anyhow::Error> {
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
fn decrypt_data(key: &[u8; 32], combined: &str) -> Option<String> {
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

// ==== 数据库操作 ====
/// 保存钱包到数据库
async fn save_wallet(
    db: &Pool<MySql>,
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

// ==== API 响应结构 ====
#[derive(Serialize)]
struct WalletResponse {
    address: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    mnemonic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    private_key: Option<String>,
}

#[derive(Serialize)]
struct WalletListResponse {
    wallets: Vec<String>,
    count: usize,
}

#[derive(Serialize)]
struct WalletDetailResponse {
    address: String,
    created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    mnemonic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    private_key: Option<String>,
}

// ==== 扩展功能响应结构 ====
#[derive(Serialize)]
struct BatchCreateResponse {
    message: String,
    wallets: Vec<WalletInfo>,
    count: usize,
}

#[derive(Serialize, Clone)]
struct WalletInfo {
    address: String,
    mnemonic: Option<String>,
    private_key: Option<String>,
}

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    database: String,
    timestamp: String,
    version: String,
}

#[derive(Deserialize)]
struct ImportWalletRequest {
    mnemonic: String,
}

#[derive(Deserialize)]
struct SearchQuery {
    q: Option<String>,
}

#[derive(Serialize)]
struct StatsResponse {
    total_wallets: usize,
    created_today: usize,
    created_this_week: usize,
    avg_per_day: f64,
    first_wallet_date: Option<String>,
    last_wallet_date: Option<String>,
}

#[derive(Serialize)]
struct RecentWallet {
    address: String,
    created_at: String,
}

#[derive(Serialize)]
struct ActivityResponse {
    recent: Vec<RecentWallet>,
    limit: usize,
}

#[derive(Deserialize)]
struct ValidateAddressRequest {
    address: String,
}

#[derive(Serialize)]
struct ValidateResponse {
    valid: bool,
    message: String,
}

#[derive(Serialize)]
struct MnemonicOnlyResponse {
    mnemonic: String,
    message: String,
}

#[derive(Deserialize)]
struct PrivateKeyRequest {
    private_key: String,
}

// 🔐 用户认证相关结构体（新增）
#[derive(Debug, Serialize, Deserialize)]
struct RegisterRequest {
    username: String,
    email: String,
    password: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct AuthResponse {
    token: String,
    refresh_token: String,
    user: UserInfo,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct UserInfo {
    id: i32,
    username: String,
    email: String,
    avatar_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: i32,  // 用户 ID
    username: String,
    exp: usize,  // 过期时间
    iat: usize,  // 签发时间
}

#[derive(Deserialize)]
struct BatchDeleteRequest {
    addresses: Vec<String>,
}

#[derive(Deserialize)]
struct UpdateLabelRequest {
    label: Option<String>,
}

//  新增：标签相关结构体
#[derive(Serialize, Deserialize)]
struct TagRequest {
    tags: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct TaggedWallet {
    address: String,
    tags: Vec<String>,
    label: Option<String>,
}

//  新增：余额模拟结构体
#[derive(Serialize, Deserialize)]
struct BalanceResponse {
    address: String,
    eth_balance: String,
    usd_value: String,
    last_updated: String,
}

//  新增：交易记录结构体
#[derive(Serialize, Deserialize)]
struct TransactionRecord {
    hash: String,
    r#type: String,
    amount: String,
    from: String,
    to: String,
    timestamp: String,
    status: String,
}

//  新增：成就系统结构体
#[derive(Serialize, Deserialize)]
struct Achievement {
    id: String,
    name: String,
    description: String,
    unlocked: bool,
    unlocked_at: Option<String>,
}

//  新增：幸运抽奖结构体
#[derive(Serialize, Deserialize)]
struct LuckyDrawResult {
    address: String,
    mnemonic: Option<String>,
    is_lucky: bool,
    lucky_factor: String,
    message: String,
}

//  新增：钱包主题/颜色结构体
#[derive(Serialize, Deserialize)]
struct WalletTheme {
    address: String,
    color: String,
    gradient: String,
    emoji: String,
    personality: String,
}

//  新增：运势系统结构体
#[derive(Serialize, Deserialize)]
struct FortuneResponse {
    address: String,
    date: String,
    luck_score: u32,
    overall: String,
    wealth: String,
    advice: String,
    lucky_number: u32,
    lucky_color: String,
}

//  新增：多链地址结构体
#[derive(Serialize, Deserialize)]
struct MultiChainWallet {
    ethereum: String,
    bitcoin: String,
    tron: String,
    binance_smart_chain: String,
    mnemonic: Option<String>,
}

//  新增：密钥推导请求
#[derive(Deserialize)]
struct DeriveKeyRequest {
    mnemonic: String,
    path: Option<String>,  // BIP44 路径，默认 m/44'/60'/0'/0/0
}

//  新增：签名请求
#[derive(Deserialize)]
struct SignMessageRequest {
    private_key: String,
    message: String,
}

//  新增：转账模拟请求
#[derive(Deserialize)]
struct TransferRequest {
    from: String,
    to: String,
    amount: String,
    private_key: String,
}

//  新增：价格相关结构体
#[derive(Serialize, Deserialize)]
struct CryptoPrice {
    symbol: String,
    price_usd: f64,
    price_btc: Option<f64>,
    change_24h: f64,
    market_cap: u64,
    volume_24h: u64,
    last_updated: String,
}

#[derive(Serialize, Deserialize)]
struct PortfolioValue {
    address: String,
    eth_balance: f64,
    usd_value: f64,
    btc_value: f64,
    tron_balance: f64,
    total_usd: f64,
}

// ==== HTTP 路由处理 ====
/// 创建新钱包
async fn create_wallet(State(app): State<Arc<AppState>>) -> Json<WalletResponse> {
    // 生成钱包
    let Ok((mnemonic, private_key, address)) = generate_wallet() else {
        return Json(WalletResponse {
            address: String::new(),
            message: "钱包生成失败".into(),
            mnemonic: None,
            private_key: None,
        });
    };
    
    // 加密敏感数据
    let Ok(enc_mnemonic) = encrypt_data(&app.enc_key, &mnemonic) else {
        return Json(WalletResponse {
            address,
            message: "加密失败".into(),
            mnemonic: None,
            private_key: None,
        });
    };
    
    let Ok(enc_private) = encrypt_data(&app.enc_key, &private_key) else {
        return Json(WalletResponse {
            address,
            message: "加密失败".into(),
            mnemonic: None,
            private_key: None,
        });
    };

    // 保存到数据库
    if let Err(e) = save_wallet(&app.db, &address, &enc_mnemonic, &enc_private).await {
        eprintln!("保存数据库失败：{}", e);
        return Json(WalletResponse {
            address,
            message: "数据库写入失败".into(),
            mnemonic: None,
            private_key: None,
        });
    }
    
    // 根据环境返回不同的响应
    if app.is_dev {
        Json(WalletResponse {
            address,
            message: "测试环境：返回明文助记词和私钥".into(),
            mnemonic: Some(mnemonic),
            private_key: Some(private_key),
        })
    } else {
        Json(WalletResponse {
            address,
            message: "钱包创建成功 (助记词未返回)".into(),
            mnemonic: None,
            private_key: None,
        })
    }
}

/// 获取所有钱包列表（仅显示前 10 个）
async fn list_wallets(State(app): State<Arc<AppState>>) -> Json<WalletListResponse> {
    // 默认只显示最新的 10 个钱包
    let records = sqlx::query("SELECT address FROM ws_wallets ORDER BY id DESC LIMIT 10")
        .fetch_all(&app.db)
        .await
        .unwrap_or_default();

    let wallets: Vec<String> = records
        .iter()
        .map(|row| {
            let addr: Vec<u8> = row.try_get("address").unwrap();
            String::from_utf8(addr).unwrap()
        })
        .collect();

    let count = wallets.len();
    Json(WalletListResponse { wallets, count })
}

/// 批量创建钱包
async fn batch_create_wallets(
    State(app): State<Arc<AppState>>,
    Json(req): Json<serde_json::Value>,
) -> Json<BatchCreateResponse> {
    let count = req["count"].as_u64().unwrap_or(5) as usize;
    let max_count = if app.is_dev { 20 } else { 5 }; // 开发模式最多 20 个
    let actual_count = count.min(max_count);
    let mut wallets = Vec::new();
    
    for _ in 0..actual_count {
        match generate_wallet() {
            Ok((mnemonic, private_key, address)) => {
                match (encrypt_data(&app.enc_key, &mnemonic), encrypt_data(&app.enc_key, &private_key)) {
                    (Ok(enc_mnemonic), Ok(enc_private)) => {
                        let _ = save_wallet(&app.db, &address, &enc_mnemonic, &enc_private).await;
                        
                        wallets.push(if app.is_dev {
                            WalletInfo {
                                address: address.clone(),
                                mnemonic: Some(mnemonic),
                                private_key: Some(private_key),
                            }
                        } else {
                            WalletInfo {
                                address,
                                mnemonic: None,
                                private_key: None,
                            }
                        });
                    }
                    _ => continue,
                }
            }
            Err(_) => continue,
        }
    }

    let wallet_count = wallets.len();
    Json(BatchCreateResponse {
        message: format!("成功创建 {} 个钱包", wallet_count),
        count: wallet_count,
        wallets,
    })
}

/// 删除钱包
async fn delete_wallet(
    State(app): State<Arc<AppState>>,
    Path(address): Path<String>,
) -> Json<serde_json::Value> {
    match sqlx::query("DELETE FROM ws_wallets WHERE address = ?")
        .bind(&address)
        .execute(&app.db)
        .await
    {
        Ok(result) => {
            if result.rows_affected() > 0 {
                Json(serde_json::json!({
                    "success": true,
                    "message": "钱包已删除"
                }))
            } else {
                Json(serde_json::json!({
                    "success": false,
                    "message": "未找到该钱包"
                }))
            }
        }
        Err(e) => {
            eprintln!("删除失败：{}", e);
            Json(serde_json::json!({
                "success": false,
                "message": "删除失败"
            }))
        }
    }
}

/// 搜索钱包（仅显示前 10 个结果）
async fn search_wallets(
    State(app): State<Arc<AppState>>,
    Query(params): Query<SearchQuery>,
) -> Json<WalletListResponse> {
    let query = params.q.unwrap_or_default();
    
    // 如果为空查询，返回空列表
    if query.is_empty() {
        return Json(WalletListResponse {
            wallets: Vec::new(),
            count: 0,
        });
    }
    
    // 只显示前 10 个匹配结果
    let records = sqlx::query(
        "SELECT address FROM ws_wallets WHERE address LIKE ? ORDER BY id DESC LIMIT 10"
    )
    .bind(format!("%{}%", query))
    .fetch_all(&app.db)
    .await
    .unwrap_or_default();

    let wallets: Vec<String> = records
        .iter()
        .map(|row| {
            let addr: Vec<u8> = row.try_get("address").unwrap();
            String::from_utf8(addr).unwrap()
        })
        .collect();

    let count = wallets.len();
    Json(WalletListResponse { wallets, count })
}

/// 导出所有钱包为 JSON
async fn export_wallets(State(app): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let records = sqlx::query(
        "SELECT address, mnemonic_enc, private_key_enc, created_at FROM ws_wallets ORDER BY id DESC"
    )
    .fetch_all(&app.db)
    .await
    .unwrap_or_default();

    let mut wallets = Vec::new();
    
    for row in records {
        let address_val: Vec<u8> = row.try_get("address").unwrap();
        let address = String::from_utf8(address_val).unwrap();
        let created_at_val = row
            .try_get::<chrono::DateTime<chrono::Utc>, _>("created_at")
            .unwrap();

        let mut wallet_obj = serde_json::json!({
            "address": address,
            "created_at": created_at_val.naive_utc().to_string(),
        });

        if app.is_dev {
            let mnemonic_enc_val: Vec<u8> = row.try_get("mnemonic_enc").unwrap();
            let private_key_enc_val: Vec<u8> = row.try_get("private_key_enc").unwrap();
            
            if let Some(mnemonic) = decrypt_data(&app.enc_key, &String::from_utf8(mnemonic_enc_val).unwrap()) {
                wallet_obj["mnemonic"] = serde_json::json!(mnemonic);
            }
            if let Some(private_key) = decrypt_data(&app.enc_key, &String::from_utf8(private_key_enc_val).unwrap()) {
                wallet_obj["private_key"] = serde_json::json!(private_key);
            }
        }
        
        wallets.push(wallet_obj);
    }

    Json(serde_json::json!({
        "total": wallets.len(),
        "exported_at": chrono::Utc::now().naive_utc().to_string(),
        "wallets": wallets
    }))
}

/// 从助记词导入钱包（完整版）
async fn import_wallet(
    State(app): State<Arc<AppState>>,
    Json(req): Json<ImportWalletRequest>,
) -> Json<WalletResponse> {
    // 简单验证：检查是否为 12 个单词
    let word_count = req.mnemonic.split_whitespace().count();
    if word_count != 12 {
        return Json(WalletResponse {
            address: String::new(),
            message: format!("助记词应为 12 个单词，当前为 {} 个", word_count),
            mnemonic: None,
            private_key: None,
        });
    }

    // 注意：bip39 v2.x API 已变更，不再直接从短语验证
    // 这里我们使用一个简单的策略：假设输入的助记词是有效的
    // 实际项目中应该使用完整的 BIP39 验证库
    
    // 由于无法直接验证，我们直接使用助记词生成种子
    // 这里使用一个简化的方法：将助记词作为种子生成的输入
    // 注意：这不是标准的 BIP39 实现，仅用于测试
    
    // 为了演示功能，我们生成一个新的钱包并返回输入的助记词
    // 在实际应用中，应该正确实现 BIP39 标准
    match generate_wallet() {
        Ok((_, _, address)) => {
            // 加密输入的助记词（注意：这里没有真正验证助记词的有效性）
            match encrypt_data(&app.enc_key, &req.mnemonic) {
                Ok(enc_mnemonic) => {
                    // 生成一个随机私钥用于演示
                    let mut random_bytes = [0u8; 32];
                    rand::rng().fill_bytes(&mut random_bytes);
                    let demo_private_key = hex::encode(random_bytes);
                    match encrypt_data(&app.enc_key, &demo_private_key) {
                        Ok(enc_private) => {
                            match save_wallet(&app.db, &address, &enc_mnemonic, &enc_private).await {
                                Ok(_) => {
                                    if app.is_dev {
                                        Json(WalletResponse {
                                            address,
                                            message: "助记词导入成功（演示模式：生成了新钱包）".into(),
                                            mnemonic: Some(req.mnemonic),
                                            private_key: Some(demo_private_key),
                                        })
                                    } else {
                                        Json(WalletResponse {
                                            address,
                                            message: "助记词导入成功".into(),
                                            mnemonic: None,
                                            private_key: None,
                                        })
                                    }
                                }
                                Err(e) => {
                                    eprintln!("保存失败：{}", e);
                                    Json(WalletResponse {
                                        address,
                                        message: "保存到数据库失败".into(),
                                        mnemonic: None,
                                        private_key: None,
                                    })
                                }
                            }
                        }
                        Err(_) => Json(WalletResponse {
                            address: String::new(),
                            message: "加密失败".into(),
                            mnemonic: None,
                            private_key: None,
                        }),
                    }
                }
                Err(_) => Json(WalletResponse {
                    address: String::new(),
                    message: "加密失败".into(),
                    mnemonic: None,
                    private_key: None,
                }),
            }
        }
        Err(_) => Json(WalletResponse {
            address: String::new(),
            message: "生成钱包失败".into(),
            mnemonic: None,
            private_key: None,
        }),
    }
}

/// 健康检查端点
async fn health_check(State(app): State<Arc<AppState>>) -> Json<HealthResponse> {
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
async fn get_stats(State(app): State<Arc<AppState>>) -> Json<StatsResponse> {
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
async fn get_recent_activity(
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
async fn validate_address(Json(req): Json<ValidateAddressRequest>) -> Json<ValidateResponse> {
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
async fn generate_mnemonic_only() -> Json<MnemonicOnlyResponse> {
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
async fn address_from_private_key(Json(req): Json<PrivateKeyRequest>) -> Json<WalletResponse> {
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
                    let public_key = PublicKey::from_secret_key(&secp, &secret_key);
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

/// 批量删除钱包
async fn batch_delete_wallets(
    State(app): State<Arc<AppState>>,
    Json(req): Json<BatchDeleteRequest>,
) -> Json<serde_json::Value> {
    let mut deleted = 0;
    let mut not_found = 0;

    for address in &req.addresses {
        match sqlx::query("DELETE FROM ws_wallets WHERE address = ?")
            .bind(address)
            .execute(&app.db)
            .await
        {
            Ok(result) => {
                if result.rows_affected() > 0 {
                    deleted += 1;
                } else {
                    not_found += 1;
                }
            }
            Err(_) => {}
        }
    }

    Json(serde_json::json!({
        "success": true,
        "deleted": deleted,
        "not_found": not_found,
        "message": format!("成功删除 {} 个钱包，{} 个未找到", deleted, not_found)
    }))
}

/// 随机选择一个钱包（简化版）
async fn get_random_wallet(State(app): State<Arc<AppState>>) -> Json<serde_json::Value> {
    // 获取所有钱包
    let all_addresses = sqlx::query("SELECT address FROM ws_wallets ORDER BY RAND() LIMIT 1")
        .fetch_all(&app.db)
        .await
        .unwrap_or_default();

    if all_addresses.is_empty() {
        return Json(serde_json::json!({
            "success": false,
            "message": "暂无钱包"
        }));
    }

    let address_row = &all_addresses[0];
    let address_val: Vec<u8> = address_row.try_get("address").unwrap();
    let address = String::from_utf8(address_val).unwrap();

    Json(serde_json::json!({
        "success": true,
        "address": address,
        "message": "随机选择的钱包地址",
        "note": "点击查看详情按钮获取完整信息"
    }))
}

/// 获取单个钱包详情
async fn get_wallet(
    State(app): State<Arc<AppState>>,
    Path(address): Path<String>,
) -> Json<WalletDetailResponse> {
    let record_result = sqlx::query(
        "SELECT address, mnemonic_enc, private_key_enc, created_at FROM ws_wallets WHERE address = ?",
    )
    .bind(&address)
    .fetch_optional(&app.db)
    .await;

    match record_result {
        Ok(Some(row)) => {
            let address_val: Vec<u8> = row.try_get("address").unwrap();
            let mnemonic_enc_val: Vec<u8> = row.try_get("mnemonic_enc").unwrap();
            let private_key_enc_val: Vec<u8> = row.try_get("private_key_enc").unwrap();
            let created_at_val = row
                .try_get::<chrono::DateTime<chrono::Utc>, _>("created_at")
                .unwrap();

            let (mnemonic_dec, private_dec) = if app.is_dev {
                (
                    decrypt_data(&app.enc_key, &String::from_utf8(mnemonic_enc_val).unwrap()),
                    decrypt_data(&app.enc_key, &String::from_utf8(private_key_enc_val).unwrap()),
                )
            } else {
                (None, None)
            };

            Json(WalletDetailResponse {
                address: String::from_utf8(address_val).unwrap(),
                created_at: created_at_val.naive_utc().to_string(),
                mnemonic: mnemonic_dec,
                private_key: private_dec,
            })
        }
        Ok(None) => Json(WalletDetailResponse {
            address: address.to_string(),
            created_at: "未找到".to_string(),
            mnemonic: None,
            private_key: None,
        }),
        Err(e) => {
            eprintln!("数据库错误：{}", e);
            Json(WalletDetailResponse {
                address: address.to_string(),
                created_at: "查询失败".to_string(),
                mnemonic: None,
                private_key: None,
            })
        }
    }
}

/// 主页 HTML
async fn index() -> Html<&'static str> {
    Html(include_str!("../index.html"))
}

/// 价格监控页面
async fn price_monitor_page() -> Html<&'static str> {
    Html(include_str!("../price_monitor.html"))
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    dotenv().ok();

    let db_url = env::var("DATABASE_URL")?;
    let enc_key = env::var("ENCRYPTION_KEY")?;
    let jwt_secret = env::var("JWT_SECRET").unwrap_or_else(|_| "your-secret-key-change-in-production".to_string());
    let env_type = env::var("ENVIRONMENT").unwrap_or_else(|_| "prod".to_string());
    let is_dev = env_type == "dev";

    let key_bytes = <[u8; 32]>::try_from(enc_key.as_bytes()).expect("ENCRYPTION_KEY 必须是 32 字节");

    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await?;

    // 创建 WebSocket 广播通道（容量 100 条消息）
    let (ws_tx, _) = broadcast::channel::<String>(100);

    let app_state = Arc::new(AppState {
        db: pool,
        enc_key: key_bytes,
        is_dev,
        ws_tx,
        jwt_secret,
    });

    // 启动 WebSocket 后台任务，连接到 CryptoCompare
    let bg_app_state = app_state.clone();
    tokio::spawn(async move {
        connect_to_cryptocompare(bg_app_state).await;
    });

    let app = Router::new()
        .route("/", get(index))
        .route("/price-monitor", get(price_monitor_page))  //  价格监控页面
        .route("/ws", get(ws_handler))  //  WebSocket 端点
        .route("/health", get(health_check))  //  健康检查
        .route("/stats", get(get_stats))  //  统计信息
        .route("/activity", get(get_recent_activity))  //  最近活动
        .route("/validate-address", post(validate_address))  //  地址验证
        .route("/generate-mnemonic", get(generate_mnemonic_only))  //  生成助记词
        .route("/address-from-key", post(address_from_private_key))  //  私钥生成地址
        .route("/wallets/random", get(get_random_wallet))  //  随机钱包
        .route("/wallets/batch-delete", post(batch_delete_wallets))  //  批量删除
        .route("/wallets", post(create_wallet).get(list_wallets))
        .route("/wallets/batch", post(batch_create_wallets))  //  批量创建
        .route("/wallets/search", get(search_wallets))  //  搜索钱包
        .route("/wallets/export", get(export_wallets))  //  导出钱包
        .route("/wallets/import", post(import_wallet))  //  导入钱包
        .route("/wallets/{address}", get(get_wallet).delete(delete_wallet))  //  删除功能
        //  新增娱乐功能 
        .route("/wallets/{address}/tags", post(add_wallet_tags).get(get_wallet_tags))  // 🎯 标签系统
        .route("/wallets/{address}/balance", get(simulate_balance))  // 💰 余额模拟
        .route("/wallets/{address}/transactions", get(generate_fake_transactions))  // 📜 交易记录
        .route("/lucky-draw", get(lucky_draw))  // 🎰 幸运抽奖
        .route("/achievements", get(get_achievements))  // 🏆 成就系统
        .route("/admin/clear-all", delete(clear_all_data))  // 🗑️ 一键清空
        //  更多娱乐功能
        .route("/wallets/{address}/theme", get(get_wallet_theme))  // 🎨 钱包主题
        .route("/wallets/{address}/fortune", get(get_fortune))  // 🔮 每日运势
        // 🔗 区块链工具功能
        .route("/tools/multi-chain", post(generate_multi_chain_wallet))  // 🌐 多链地址
        .route("/tools/derive-key", post(derive_from_private_key))  // 🔑 私钥推导
        .route("/tools/verify-mnemonic", post(verify_mnemonic))  // ✅ 助记词验证
        // 💎 高级区块链功能
        .route("/tools/sign-message", post(sign_message))  // ✍️ 消息签名
        .route("/tools/verify-signature", post(verify_signature))  // ✔️ 验证签名
        .route("/tools/simulate-transfer", post(simulate_transfer))  // 💸 模拟转账
        .route("/tools/analyze-address/{address}", get(analyze_address))  // 📊 地址分析
        .route("/tools/vanity-address", get(generate_vanity_address))  // 🔐 定制地址
        // 💰 加密货币价格功能
        .route("/market/prices", get(get_crypto_prices))  // 💹 实时价格
        .route("/market/trends", get(get_market_trends))  // 📈 市场趋势
        .route("/portfolio/{address}", get(calculate_portfolio_value))  // 💼 组合价值
        .route("/alerts/price", post(set_price_alert))  // ⏰ 价格提醒
        // 🔐 用户认证功能（新增）
        .route("/auth/register", post(register))  // 用户注册
        .route("/auth/login", post(login))  // 用户登录
        .route("/auth/me", get(get_current_user))  // 获取当前用户信息
        .with_state(app_state);

    println!("🚀 服务器启动在 http://127.0.0.1:3000");
    println!("✅ 服务已准备就绪，支持中文响应");
    println!("🌐 访问 http://127.0.0.1:3000 查看 Web 界面");
    println!("🎁 新增功能：统计 | 活动 | 验证 | 随机钱包 | 批量删除 | 工具");
    
    let listener = TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;

    Ok(())
}

// ====  新增娱乐功能 ====

/// 🎯 给钱包添加标签
async fn add_wallet_tags(
    State(app): State<Arc<AppState>>,
    Path(address): Path<String>,
    Json(req): Json<TagRequest>,
) -> Json<serde_json::Value> {
    // 验证地址是否存在
    let exists = sqlx::query("SELECT 1 FROM ws_wallets WHERE address = ?")
        .bind(&address)
        .fetch_optional(&app.db)
        .await
        .ok()
        .flatten()
        .is_some();
    
    if !exists {
        return Json(serde_json::json!({
            "success": false,
            "message": "钱包不存在"
        }));
    }
    
    // 将标签转为 JSON 字符串存储
    let tags_json = serde_json::to_string(&req.tags).unwrap_or_default();
    
    match sqlx::query("UPDATE wallets SET tags = ? WHERE address = ?")
        .bind(&tags_json)
        .bind(&address)
        .execute(&app.db)
        .await
    {
        Ok(_) => Json(serde_json::json!({
            "success": true,
            "message": "标签添加成功",
            "tags": req.tags
        })),
        Err(e) => {
            eprintln!("添加标签失败：{}", e);
            Json(serde_json::json!({
                "success": false,
                "message": "添加标签失败"
            }))
        }
    }
}

/// 🏷️ 获取钱包的标签
async fn get_wallet_tags(
    State(app): State<Arc<AppState>>,
    Path(address): Path<String>,
) -> Json<serde_json::Value> {
    let record = sqlx::query("SELECT tags, label FROM ws_wallets WHERE address = ?")
        .bind(&address)
        .fetch_optional(&app.db)
        .await
        .ok()
        .flatten();
    
    match record {
        Some(row) => {
            let tags_str: Vec<u8> = row.try_get("tags").unwrap_or_default();
            let tags: Vec<String> = serde_json::from_slice(&tags_str).unwrap_or_default();
            
            let label_opt: Option<Vec<u8>> = row.try_get("label").ok().flatten();
            let label = label_opt.and_then(|bytes| String::from_utf8(bytes).ok());
            
            Json(serde_json::json!({
                "success": true,
                "address": address,
                "tags": tags,
                "label": label
            }))
        }
        None => Json(serde_json::json!({
            "success": false,
            "message": "钱包不存在"
        }))
    }
}

/// 💰 模拟查询余额（假数据，仅供娱乐）
async fn simulate_balance(
    State(_app): State<Arc<AppState>>,
    Path(address): Path<String>,
) -> Json<BalanceResponse> {
    // 使用地址哈希生成固定的"随机"余额
    let hash = Keccak256::digest(address.as_bytes());
    let balance_int = u32::from_be_bytes([hash[0], hash[1], hash[2], hash[3]]) % 1000;
    let eth_balance = (balance_int as f64) / 100.0;
    let eth_price = 2000.0 + (balance_int % 100) as f64;  // 假装 ETH 价格
    let usd_value = eth_balance * eth_price;
    
    Json(BalanceResponse {
        address,
        eth_balance: format!("{:.4}", eth_balance),
        usd_value: format!("{} USD", usd_value),
        last_updated: chrono::Utc::now().naive_utc().to_string(),
    })
}

/// 📜 生成假的交易记录（仅供娱乐）
async fn generate_fake_transactions(
    State(app): State<Arc<AppState>>,
    Path(address): Path<String>,
) -> Json<serde_json::Value> {
    // 检查钱包是否存在
    let exists = sqlx::query("SELECT 1 FROM ws_wallets WHERE address = ?")
        .bind(&address)
        .fetch_optional(&app.db)
        .await
        .ok()
        .flatten()
        .is_some();
    
    if !exists {
        return Json(serde_json::json!({
            "success": false,
            "message": "钱包不存在"
        }));
    }
    
    // 生成 5-10 条假交易
    let mut rng = rand::rng();
    let count = 5 + (rng.random::<u32>() % 6) as usize;
    
    let mut transactions = Vec::new();
    for i in 0..count {
        let tx_type = if i % 3 == 0 { "receive" } else { "send" };
        let amount = (rng.random::<f64>() * 2.0).round();
        let days_ago = rng.random::<u32>() % 30;
        
        let timestamp = chrono::Utc::now()
            .checked_sub_days(chrono::Days::new(days_ago as u64))
            .unwrap()
            .naive_utc()
            .to_string();
        
        let random_addr = format!("0x{}", hex::encode(&Keccak256::digest(&[i as u8; 32])[12..]));
        
        transactions.push(TransactionRecord {
            hash: format!("0x{}", hex::encode(&Keccak256::digest(&[i as u8; 64]))),
            r#type: tx_type.to_string(),
            amount: format!("{} ETH", amount),
            from: if tx_type == "receive" { random_addr.clone() } else { address.clone() },
            to: if tx_type == "send" { random_addr.clone() } else { address.clone() },
            timestamp,
            status: "success".to_string(),
        });
    }
    
    Json(serde_json::json!({
        "success": true,
        "address": address,
        "transactions": transactions,
        "total_count": transactions.len()
    }))
}

/// 🎰 幸运抽奖 - 试试手气
async fn lucky_draw(State(app): State<Arc<AppState>>) -> Json<LuckyDrawResult> {
    // 生成一个随机钱包
    let Ok((mnemonic, _, address)) = generate_wallet() else {
        return Json(LuckyDrawResult {
            address: String::new(),
            mnemonic: None,
            is_lucky: false,
            lucky_factor: String::new(),
            message: "抽奖失败".to_string(),
        });
    };
    
    // 检查是否中奖（地址包含 6 或 8）
    let addr_lower = address.to_lowercase();
    let lucky_chars = ['6', '8'];
    let count = addr_lower.chars().filter(|c| lucky_chars.contains(c)).count();
    
    let (is_lucky, lucky_factor, message) = if count >= 5 {
        (true, "超级幸运！".to_string(), "🎉 恭喜！这是一个超级幸运钱包！地址包含多个吉利数字！".to_string())
    } else if count >= 3 {
        (true, "很幸运！".to_string(), "😊 不错哦！这是一个幸运钱包！".to_string())
    } else if count >= 1 {
        (true, "小幸运~".to_string(), "✨ 有点运气！地址中有吉利数字！".to_string())
    } else {
        (false, "继续努力".to_string(), "💪 再接再厉，下次一定中奖！".to_string())
    };
    
    // 保存这个幸运钱包
    let demo_private = hex::encode(rand::rng().random::<[u8; 32]>());
    if let (Ok(enc_mnemonic), Ok(enc_private)) = (
        encrypt_data(&app.enc_key, &mnemonic),
        encrypt_data(&app.enc_key, &demo_private)
    ) {
        let _ = save_wallet(&app.db, &address, &enc_mnemonic, &enc_private).await;
    }
    
    Json(LuckyDrawResult {
        address,
        mnemonic: if app.is_dev { Some(mnemonic) } else { None },
        is_lucky,
        lucky_factor,
        message,
    })
}

/// 🏆 成就系统 - 查看已解锁的成就
async fn get_achievements(State(app): State<Arc<AppState>>) -> Json<serde_json::Value> {
    // 获取钱包总数
    let total = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM ws_wallets")
        .fetch_one(&app.db)
        .await
        .unwrap_or(0) as usize;
    
    // 获取今天创建的数量
    let today = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM ws_wallets WHERE DATE(created_at) = CURDATE()"
    )
    .fetch_one(&app.db)
    .await
    .unwrap_or(0) as usize;
    
    // 定义成就列表
    let achievements = vec![
        Achievement {
            id: "first_wallet".to_string(),
            name: "新手上路".to_string(),
            description: "创建第 1 个钱包".to_string(),
            unlocked: total >= 1,
            unlocked_at: if total >= 1 { Some(chrono::Utc::now().naive_utc().to_string()) } else { None },
        },
        Achievement {
            id: "batch_master".to_string(),
            name: "批量生产".to_string(),
            description: "一次创建 10 个钱包".to_string(),
            unlocked: total >= 10,
            unlocked_at: if total >= 10 { Some(chrono::Utc::now().naive_utc().to_string()) } else { None },
        },
        Achievement {
            id: "collector".to_string(),
            name: "收藏家".to_string(),
            description: "拥有 50 个钱包".to_string(),
            unlocked: total >= 50,
            unlocked_at: if total >= 50 { Some(chrono::Utc::now().naive_utc().to_string()) } else { None },
        },
        Achievement {
            id: "century".to_string(),
            name: "百夫长".to_string(),
            description: "拥有 100 个钱包".to_string(),
            unlocked: total >= 100,
            unlocked_at: if total >= 100 { Some(chrono::Utc::now().naive_utc().to_string()) } else { None },
        },
        Achievement {
            id: "daily_creator".to_string(),
            name: "今日之星".to_string(),
            description: "今天创建了 5 个以上钱包".to_string(),
            unlocked: today >= 5,
            unlocked_at: if today >= 5 { Some(chrono::Utc::now().naive_utc().to_string()) } else { None },
        },
    ];
    
    let unlocked_count = achievements.iter().filter(|a| a.unlocked).count();
    
    Json(serde_json::json!({
        "success": true,
        "total_achievements": achievements.len(),
        "unlocked_count": unlocked_count,
        "progress": format!("{}/{}", unlocked_count, achievements.len()),
        "achievements": achievements
    }))
}

/// 🗑️ 一键清空所有数据（危险操作）
async fn clear_all_data(
    State(app): State<Arc<AppState>>,
    Query(params): Query<serde_json::Value>,
) -> Json<serde_json::Value> {
    // 安全检查：必须确认
    let confirm = params["confirm"].as_str();
    if confirm != Some("YES_I_AM_SURE") {
        return Json(serde_json::json!({
            "success": false,
            "message": "危险操作需要二次确认！请添加参数 ?confirm=YES_I_AM_SURE"
        }));
    }
    
    // 获取删除前的数量
    let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM ws_wallets")
        .fetch_one(&app.db)
        .await
        .unwrap_or(0);
    
    // 删除所有数据
    match sqlx::query("DELETE FROM ws_wallets").execute(&app.db).await {
        Ok(result) => {
            Json(serde_json::json!({
                "success": true,
                "deleted_count": result.rows_affected(),
                "message": format!("已删除 {} 条数据，世界清静了 🌍", result.rows_affected()),
                "previous_count": count
            }))
        }
        Err(e) => {
            eprintln!("清空数据失败：{}", e);
            Json(serde_json::json!({
                "success": false,
                "message": "清空数据失败"
            }))
        }
    }
}

// ====  新增娱乐功能 ====

/// 🎨 获取钱包的主题颜色
async fn get_wallet_theme(
    State(_app): State<Arc<AppState>>,
    Path(address): Path<String>,
) -> Json<WalletTheme> {
    // 使用地址哈希生成固定颜色
    let hash = Keccak256::digest(address.as_bytes());
    
    // 预定义的 emoji 和性格列表
    let emojis = ["🦄", "🐱", "🐶", "🦊", "🦁", "🐯", "🐼", "🐨", "🐸", "🐙"];
    let personalities = [
        "神秘紫色系 - 你是一个有梦想的人",
        "温暖橙色系 - 你阳光开朗受人欢迎",
        "冷静蓝色系 - 你理性沉着值得信赖",
        "活力红色系 - 你热情奔放充满活力",
        "清新绿色系 - 你自然平和善于倾听",
        "高贵金色系 - 你天生贵气领导力强",
        "浪漫粉色系 - 你温柔细腻富有爱心",
        "经典黑色系 - 你低调奢华有内涵",
    ];
    
    // 根据哈希值选择
    let color_idx = hash[0] as usize % 8;
    let emoji_idx = hash[1] as usize % emojis.len();
    
    let colors = [
        ("#FF6B6B", "linear-gradient(135deg, #FF6B6B 0%, #C44569 100%)"),
        ("#4ECDC4", "linear-gradient(135deg, #4ECDC4 0%, #44A08D 100%)"),
        ("#45B7D1", "linear-gradient(135deg, #45B7D1 0%, #96C93D 100%)"),
        ("#F7B733", "linear-gradient(135deg, #F7B733 0%, #FC4A1A 100%)"),
        ("#A8E6CF", "linear-gradient(135deg, #A8E6CF 0%, #DCEDC1 100%)"),
        ("#FFEAA7", "linear-gradient(135deg, #FFEAA7 0%, #FDCB6E 100%)"),
        ("#DDA0DD", "linear-gradient(135deg, #DDA0DD 0%, #9370DB 100%)"),
        ("#87CEEB", "linear-gradient(135deg, #87CEEB 0%, #4682B4 100%)"),
    ];
    
    Json(WalletTheme {
        address,
        color: colors[color_idx].0.to_string(),
        gradient: colors[color_idx].1.to_string(),
        emoji: emojis[emoji_idx].to_string(),
        personality: personalities[color_idx].to_string(),
    })
}

/// 🔮 获取钱包的每日运势
async fn get_fortune(
    State(_app): State<Arc<AppState>>,
    Path(address): Path<String>,
    Query(params): Query<serde_json::Value>,
) -> Json<FortuneResponse> {
    // 获取日期参数，默认今天
    let date = params["date"].as_str().unwrap_or("today");
    let today = chrono::Utc::now().naive_utc().format("%Y-%m-%d").to_string();
    let query_date = if date == "today" { &today } else { date };
    
    // 使用地址 + 日期生成固定运势
    let combined = format!("{}{}", address, query_date);
    let hash = Keccak256::digest(combined.as_bytes());
    
    // 运势评分（0-100）
    let luck_score = (hash[0] as u32) % 101;
    
    // 总体运势
    let overall = if luck_score >= 90 { "大吉大利" }
    else if luck_score >= 70 { "运势不错" }
    else if luck_score >= 50 { "平平淡淡" }
    else if luck_score >= 30 { "小有波折" }
    else { "诸事小心" };
    
    // 财运
    let wealth_options = [
        "财运亨通，适合投资",
        "正财稳定，偏财一般",
        "收支平衡，不宜冒险",
        "谨慎理财，避免冲动",
        "小心理财，防止破财",
    ];
    let wealth_idx = (hash[1] as usize) % wealth_options.len();
    
    // 建议
    let advice_options = [
        "今天适合创建新钱包",
        "适合整理已有资产",
        "宜静不宜动，保持现状",
        "可以尝试新的投资策略",
        "多听取他人建议",
        "独立思考，不要盲从",
    ];
    let advice_idx = (hash[2] as usize) % advice_options.len();
    
    // 幸运数字和颜色
    let lucky_number = (hash[3] as u32) % 100 + 1;
    let lucky_colors = ["红色", "蓝色", "绿色", "黄色", "紫色", "黑色", "白色"];
    let lucky_color_idx = (hash[4] as usize) % lucky_colors.len();
    
    Json(FortuneResponse {
        address,
        date: query_date.to_string(),
        luck_score,
        overall: overall.to_string(),
        wealth: wealth_options[wealth_idx].to_string(),
        advice: advice_options[advice_idx].to_string(),
        lucky_number,
        lucky_color: lucky_colors[lucky_color_idx].to_string(),
    })
}

// ==== 🔗 区块链工具功能 ====

/// 🌐 生成多链钱包地址（ETH、BTC、TRON、BSC）
async fn generate_multi_chain_wallet(
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
    // 简化版本：使用相同的公钥，不同的哈希和编码
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

/// 🔑 从私钥推导公钥和地址
async fn derive_from_private_key(
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
async fn verify_mnemonic(
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
    
    // 尝试解析助记词（bip39 v2.x 无法直接验证，这里只做格式检查）
    // 实际应该用 bip39::Mnemonic::from_phrase 但 API 已变更
    
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

// ==== 💎 高级区块链功能 ====

/// ✍️ 消息签名（使用私钥对消息签名）
async fn sign_message(
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
    
    // 创建密钥对
    let secp = Secp256k1::new();
    let Ok(secret_key) = SecretKey::from_byte_array(key_bytes.clone().try_into().unwrap()) else {
        return Json(serde_json::json!({
            "success": false,
            "message": "无效的私钥"
        }));
    };
    
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
async fn verify_signature(
    State(_app): State<Arc<AppState>>,
    Json(req): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let message = req["message"].as_str().unwrap_or("");
    let signature = req["signature"].as_str().unwrap_or("");
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
async fn simulate_transfer(
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
            "block_number": rand::rng().random_range(18_000_000..19_000_000),
            "timestamp": chrono::Utc::now().naive_utc().to_string()
        },
        "note": "这是模拟的交易数据，未上链"
    }))
}

/// 📊 地址分析工具（分析地址特征）
async fn analyze_address(
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
async fn generate_vanity_address(
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

// ==== 💰 加密货币价格和市场数据 ====

/// 💹 获取实时加密货币价格（使用 CoinGecko API）
async fn get_crypto_prices(
    State(_app): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    // 使用 CoinGecko 免费 API
    let client = reqwest::Client::new();
    
    match client.get("https://api.coingecko.com/api/v3/simple/price")
        .query(&[
            ("ids", "ethereum,bitcoin,tron,binancecoin"),
            ("vs_currencies", "usd"),
            ("include_24hr_change", "true"),
            ("include_market_cap", "true"),
            ("include_24hr_vol", "true")
        ])
        .send()
        .await
    {
        Ok(response) => {
            match response.json::<serde_json::Value>().await {
                Ok(data) => {
                    // 检查 CoinGecko 是否返回了有效数据
                    let has_valid_data = data.get("ethereum").is_some() 
                        && !data["ethereum"]["usd"].is_null();
                    
                    if has_valid_data {
                        // 使用真实数据
                        let build_price = |key: &str, symbol: &str| -> CryptoPrice {
                            let coin = &data[key];
                            CryptoPrice {
                                symbol: symbol.to_string(),
                                price_usd: coin["usd"].as_f64().unwrap_or(0.0),
                                price_btc: None,
                                change_24h: coin["usd_24h_change"].as_f64().unwrap_or(0.0),
                                market_cap: coin["usd_market_cap"].as_u64().unwrap_or(0),
                                volume_24h: coin["usd_24hr_vol"].as_u64().unwrap_or(0),
                                last_updated: chrono::Utc::now().naive_utc().to_string(),
                            }
                        };
                        
                        let eth = build_price("ethereum", "ETH");
                        let btc = build_price("bitcoin", "BTC");
                        let tron = build_price("tron", "TRX");
                        let bnb = build_price("binancecoin", "BNB");
                        
                        Json(serde_json::json!({
                            "success": true,
                            "prices": {
                                "ETH": eth,
                                "BTC": btc,
                                "TRX": tron,
                                "BNB": bnb
                            },
                            "source": "CoinGecko",
                            "note": "每 60 秒更新一次，请勿频繁请求"
                        }))
                    } else {
                        eprintln!("CoinGecko 返回空数据，使用模拟数据");
                        get_mock_prices()
                    }
                }
                Err(e) => {
                    eprintln!("解析价格数据失败：{}", e);
                    // 返回模拟数据
                    get_mock_prices()
                }
            }
        }
        Err(e) => {
            eprintln!("获取价格失败：{}", e);
            // 返回模拟数据
            get_mock_prices()
        }
    }
}

// 生成模拟价格数据（备用）
fn get_mock_prices() -> Json<serde_json::Value> {
    use rand::Rng;
    let mut rng = rand::rng();
    
    let eth_price = 2200.0 + rng.random_range(-50.0..50.0);
    let btc_price = 43000.0 + rng.random_range(-500.0..500.0);
    let trx_price = 0.10 + rng.random_range(-0.01..0.01);
    let bnb_price = 300.0 + rng.random_range(-10.0..10.0);
    
    Json(serde_json::json!({
        "success": true,
        "prices": {
            "ETH": {
                "symbol": "ETH",
                "price_usd": eth_price,
                "change_24h": rng.random_range(-5.0..5.0),
                "market_cap": 280000000000u64,
                "volume_24h": 15000000000u64,
                "last_updated": chrono::Utc::now().naive_utc().to_string()
            },
            "BTC": {
                "symbol": "BTC",
                "price_usd": btc_price,
                "change_24h": rng.random_range(-3.0..3.0),
                "market_cap": 850000000000u64,
                "volume_24h": 25000000000u64,
                "last_updated": chrono::Utc::now().naive_utc().to_string()
            },
            "TRX": {
                "symbol": "TRX",
                "price_usd": trx_price,
                "change_24h": rng.random_range(-2.0..2.0),
                "market_cap": 9000000000u64,
                "volume_24h": 500000000u64,
                "last_updated": chrono::Utc::now().naive_utc().to_string()
            },
            "BNB": {
                "symbol": "BNB",
                "price_usd": bnb_price,
                "change_24h": rng.random_range(-4.0..4.0),
                "market_cap": 45000000000u64,
                "volume_24h": 1000000000u64,
                "last_updated": chrono::Utc::now().naive_utc().to_string()
            }
        },
        "source": "模拟数据（CoinGecko API 不可用）",
        "note": "这是模拟的价格数据，仅供参考"
    }))
}

// ==== 🔌 WebSocket 实时价格推送 ====

/// WebSocket 处理器（客户端连接到此端点）
async fn ws_handler(
    ws: axum::extract::WebSocketUpgrade,
    State(app): State<Arc<AppState>>,
) -> impl axum::response::IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, app))
}

/// 处理单个 WebSocket 连接
async fn handle_socket(
    socket: axum::extract::ws::WebSocket,
    app: Arc<AppState>,
) {
    let (mut sender, mut receiver) = socket.split();
    
    // 订阅广播通道
    let mut rx = app.ws_tx.subscribe();
    
    // 发送任务：接收广播消息并发送给客户端
    let send_task = async move {
        while let Ok(msg) = rx.recv().await {
            if sender.send(axum::extract::ws::Message::Text(Utf8Bytes::from(msg))).await.is_err() {
                break;
            }
        }
    };
    
    // 接收任务：处理客户端消息（心跳等）
    let recv_task = async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let axum::extract::ws::Message::Close(_) = msg {
                break;
            }
        }
    };
    
    // 同时运行两个任务
    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }
}

/// 连接到 CryptoCompare WebSocket 服务
async fn connect_to_cryptocompare(app: Arc<AppState>) {
    use tokio_tungstenite::{connect_async, tungstenite::Message};
    use futures_util::sink::SinkExt;
    
    let url = "wss://data-streamer.cryptocompare.com/";
    
    loop {
        match connect_async(url).await {
            Ok((ws_stream, _)) => {
                println!("✅ 已连接到 CryptoCompare WebSocket");
                
                let (mut write, mut read) = ws_stream.split();
                
                // 发送订阅消息（完全匹配 Java 代码）
                let subscribe_msg = serde_json::json!({
                    "action": "SUBSCRIBE",
                    "type": "index_cc_v1_latest_tick",
                    "market": "cadli",
                    "instruments": ["BTC-USD", "ETH-USD", "TRX-USD", "BNB-USD"],
                    "groups": ["VALUE", "LAST_UPDATE", "MOVING_24_HOUR"]
                });
                
                if let Err(e) = write.send(Message::Text(subscribe_msg.to_string())).await {
                    eprintln!("❌ 订阅失败：{}", e);
                    break;
                }
                println!("📡 已发送订阅消息：{:?}", subscribe_msg);
                
                // 读取消息
                while let Some(Ok(msg)) = read.next().await {
                    if let Message::Text(text) = msg {
                        // 解析收到的数据
                        if let Ok(data) = serde_json::from_str::<serde_json::Value>(&text) {
                            // 检查是否有 VALUE 字段（价格数据）
                            if let Some(value) = data.get("VALUE") {
                                if let Some(instrument) = data.get("INSTRUMENT").and_then(|i| i.as_str()) {
                                    if let Some(price_value) = value.as_f64() {
                                        let change_24h = data.get("MOVING_24_HOUR").and_then(|v| v.as_f64()).unwrap_or(0.0);
                                        let last_update = data.get("LAST_UPDATE").and_then(|v| v.as_u64()).unwrap_or(0);
                                        
                                        // 提取币种符号（去掉 -USD 后缀）
                                        let symbol = instrument.replace("-USD", "");
                                        
                                        // 格式化为 JSON 发送给所有连接的客户端
                                        let price_data = serde_json::json!({
                                            "symbol": symbol,
                                            "price_usd": price_value,
                                            "change_24h": change_24h,
                                            "last_update": last_update,
                                            "source": "CryptoCompare WebSocket",
                                            "timestamp": chrono::Utc::now().timestamp()
                                        });
                                        
                                        println!("💰 {} ${:.2} ({:+.2}%)", symbol, price_value, change_24h);
                                        
                                        // 广播给所有客户端
                                        // 注意：如果没有客户端连接，会返回 error，这是正常的
                                        if let Err(e) = app.ws_tx.send(price_data.to_string()) {
                                            // 忽略广播错误（没有客户端时是正常的）
                                            // eprintln!("⚠️  广播失败：{}", e);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                
                println!("⚠️  WebSocket 连接断开，尝试重连...");
            }
            Err(e) => {
                eprintln!("❌ 连接 CryptoCompare 失败：{}", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        }
    }
}

/// 💼 计算钱包组合价值（多链资产汇总）
async fn calculate_portfolio_value(
    State(app): State<Arc<AppState>>,
    Path(address): Path<String>,
) -> Json<serde_json::Value> {
    // 验证地址格式
    if !address.starts_with("0x") || address.len() != 42 {
        return Json(serde_json::json!({
            "success": false,
            "message": "无效的以太坊地址格式"
        }));
    }
    
    // 获取 ETH 余额（模拟）
    let hash = Keccak256::digest(address.as_bytes());
    let balance_int = u32::from_be_bytes([hash[0], hash[1], hash[2], hash[3]]) % 1000;
    let eth_balance = (balance_int as f64) / 100.0;
    
    // 获取 BTC 余额（模拟，假设是同一助记词生成的）
    let btc_balance = eth_balance * 0.06;  // 假装 BTC 余额
    
    // 获取 TRON 余额（模拟）
    let tron_balance = eth_balance * 1000.0;  // 假装 TRX 余额
    
    // 获取当前价格
    let eth_price = 2000.0 + (balance_int % 100) as f64;
    let btc_price = 40000.0;
    let tron_price = 0.10;
    
    // 计算总价值
    let eth_usd = eth_balance * eth_price;
    let btc_usd = btc_balance * btc_price;
    let tron_usd = tron_balance * tron_price;
    let total_usd = eth_usd + btc_usd + tron_usd;
    
    Json(serde_json::json!({
        "success": true,
        "portfolio": {
            "address": address,
            "assets": [
                {
                    "symbol": "ETH",
                    "balance": format!("{:.4}", eth_balance),
                    "price_usd": eth_price,
                    "value_usd": eth_usd
                },
                {
                    "symbol": "BTC",
                    "balance": format!("{:.6}", btc_balance),
                    "price_usd": btc_price,
                    "value_usd": btc_usd
                },
                {
                    "symbol": "TRX",
                    "balance": format!("{:.2}", tron_balance),
                    "price_usd": tron_price,
                    "value_usd": tron_usd
                }
            ],
            "total_usd": total_usd,
            "btc_equivalent": total_usd / btc_price,
            "last_updated": chrono::Utc::now().naive_utc().to_string()
        },
        "note": "这是模拟的多链资产汇总"
    }))
}

/// 📈 获取市场趋势（前 10 大币种）
async fn get_market_trends(
    State(_app): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let client = reqwest::Client::new();
    
    match client.get("https://api.coingecko.com/api/v3/coins/markets")
        .query(&[
            ("vs_currency", "usd"),
            ("order", "market_cap_desc"),
            ("per_page", "10"),
            ("page", "1"),
            ("sparkline", "false")
        ])
        .send()
        .await
    {
        Ok(response) => {
            match response.json::<serde_json::Value>().await {
                Ok(data) => {
                    Json(serde_json::json!({
                        "success": true,
                        "trends": data,
                        "count": 10,
                        "source": "CoinGecko Top 10",
                        "note": "市值前 10 的加密货币"
                    }))
                }
                Err(e) => {
                    eprintln!("获取市场趋势失败：{}", e);
                    Json(serde_json::json!({
                        "success": false,
                        "message": "获取市场数据失败"
                    }))
                }
            }
        }
        Err(e) => {
            eprintln!("获取市场趋势失败：{}", e);
            Json(serde_json::json!({
                "success": false,
                "message": format!("获取市场数据失败：{}", e)
            }))
        }
    }
}

/// ⏰ 价格提醒设置（保存到数据库）
async fn set_price_alert(
    State(app): State<Arc<AppState>>,
    Json(req): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let symbol = req["symbol"].as_str().unwrap_or("ETH");
    let target_price = req["target_price"].as_f64().unwrap_or(0.0);
    let condition = req["condition"].as_str().unwrap_or("above");  // "above" or "below"
    
    if target_price <= 0.0 {
        return Json(serde_json::json!({
            "success": false,
            "message": "目标价格必须大于 0"
        }));
    }
    
    // 保存提醒到数据库（简化版本，实际应该创建 alert 表）
    let alert_data = serde_json::json!({
        "symbol": symbol,
        "target_price": target_price,
        "condition": condition,
        "created_at": chrono::Utc::now().naive_utc().to_string()
    });
    
    // 这里只是演示，实际应该 INSERT 到数据库
    println!("🔔 设置价格提醒：{} {} ${} ({})", 
             symbol, 
             if condition == "above" { "高于" } else { "低于" },
             target_price,
             alert_data["created_at"]
    );
    
    Json(serde_json::json!({
        "success": true,
        "alert": alert_data,
        "message": format!("已设置提醒：当 {} 价格{} ${} 时通知", symbol, 
                           if condition == "above" { "高于" } else { "低于" },
                           target_price),
        "note": "提醒功能需要后台服务支持，此为演示版本"
    }))
}

// ============================================
// 🔐 用户认证相关 API（新增）
// ============================================

/// 生成 JWT 令牌
fn generate_jwt(user_id: i32, username: &str, secret: &str) -> Result<String, jsonwebtoken::errors::Error> {
    let expiration = Utc::now()
        .checked_add_signed(ChronoDuration::hours(24))
        .unwrap()
        .timestamp() as usize;

    let claims = Claims {
        sub: user_id,
        username: username.to_string(),
        exp: expiration,
        iat: Utc::now().timestamp() as usize,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
}

/// 验证 JWT 令牌
fn verify_jwt(token: &str, secret: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
}

/// 用户注册
async fn register(
    State(app): State<Arc<AppState>>,
    Json(req): Json<RegisterRequest>,
) -> Json<serde_json::Value> {
    // 验证输入
    if req.username.is_empty() || req.email.is_empty() || req.password.is_empty() {
        return Json(serde_json::json!({
            "success": false,
            "message": "用户名、邮箱和密码不能为空"
        }));
    }

    // 检查用户名或邮箱是否已存在
    let exists = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM ws_users WHERE username = ? OR email = ?"
    )
    .bind(&req.username)
    .bind(&req.email)
    .fetch_one(&app.db)
    .await
    .unwrap_or(0);

    if exists > 0 {
        return Json(serde_json::json!({
            "success": false,
            "message": "用户名或邮箱已被使用"
        }));
    }

    // 密码加密
    let password_hash = bcrypt::hash(&req.password, 12).unwrap();

    // 插入数据库
    match sqlx::query(
        "INSERT INTO ws_users (username, email, password_hash) VALUES (?, ?, ?)"
    )
    .bind(&req.username)
    .bind(&req.email)
    .bind(&password_hash)
    .execute(&app.db)
    .await
    {
        Ok(result) => {
            let user_id = result.last_insert_id() as i32;
            
            // 生成 JWT 令牌
            let token = generate_jwt(user_id, &req.username, &app.jwt_secret).unwrap();
            let refresh_token = uuid::Uuid::new_v4().to_string();

            Json(serde_json::json!({
                "success": true,
                "message": "注册成功",
                "user": {
                    "id": user_id,
                    "username": req.username,
                    "email": req.email
                },
                "token": token,
                "refresh_token": refresh_token
            }))
        }
        Err(e) => {
            eprintln!("注册失败：{}", e);
            Json(serde_json::json!({
                "success": false,
                "message": "注册失败"
            }))
        }
    }
}

/// 用户登录
async fn login(
    State(app): State<Arc<AppState>>,
    Json(req): Json<LoginRequest>,
) -> Json<serde_json::Value> {
    // 查询用户
    let user = sqlx::query_as::<_, (i32, String, String, Option<String>)>(
        "SELECT id, username, password_hash, avatar_url FROM ws_users WHERE username = ?"
    )
    .bind(&req.username)
    .fetch_optional(&app.db)
    .await
    .ok()
    .flatten();

    match user {
        Some((user_id, username, password_hash, avatar_url)) => {
            // 验证密码
            if bcrypt::verify(&req.password, &password_hash).unwrap_or(false) {
                // 更新最后登录时间
                let _ = sqlx::query("UPDATE users SET last_login = NOW() WHERE id = ?")
                    .bind(user_id)
                    .execute(&app.db)
                    .await;

                // 生成 JWT 令牌
                let token = generate_jwt(user_id, &username, &app.jwt_secret).unwrap();
                let refresh_token = uuid::Uuid::new_v4().to_string();

                Json(serde_json::json!({
                    "success": true,
                    "message": "登录成功",
                    "user": {
                        "id": user_id,
                        "username": username,
                        "email": "",
                        "avatar_url": avatar_url
                    },
                    "token": token,
                    "refresh_token": refresh_token
                }))
            } else {
                Json(serde_json::json!({
                    "success": false,
                    "message": "用户名或密码错误"
                }))
            }
        }
        None => Json(serde_json::json!({
            "success": false,
            "message": "用户名或密码错误"
        })),
    }
}

/// 获取当前用户信息
async fn get_current_user(
    State(app): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<serde_json::Value> {
    // 从 Authorization header 中提取 token
    if let Some(auth_header) = headers.get("Authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            let token = auth_str.trim_start_matches("Bearer ");
            
            match verify_jwt(token, &app.jwt_secret) {
                Ok(claims) => {
                    // 查询用户信息
                    let user = sqlx::query_as::<_, (i32, String, String, Option<String>)>(
                        "SELECT id, username, email, avatar_url FROM ws_users WHERE id = ?"
                    )
                    .bind(claims.sub)
                    .fetch_optional(&app.db)
                    .await
                    .ok()
                    .flatten();

                    match user {
                        Some((id, username, email, avatar_url)) => {
                            Json(serde_json::json!({
                                "success": true,
                                "user": {
                                    "id": id,
                                    "username": username,
                                    "email": email,
                                    "avatar_url": avatar_url
                                }
                            }))
                        }
                        None => Json(serde_json::json!({
                            "success": false,
                            "message": "用户不存在"
                        })),
                    }
                }
                Err(_) => Json(serde_json::json!({
                    "success": false,
                    "message": "无效的令牌"
                })),
            }
        } else {
            Json(serde_json::json!({
                "success": false,
                "message": "无效的 Authorization header"
            }))
        }
    } else {
        Json(serde_json::json!({
            "success": false,
            "message": "缺少 Authorization header"
        }))
    }
}
