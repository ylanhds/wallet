use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
// 添加 tokio net 的引用
use serde::Serialize;
use tokio::net::TcpListener;
// bip39 用于生成助记词
use bip39::{Language, Mnemonic};
// secp256k1 用于生成公私钥对
use secp256k1::{PublicKey, Secp256k1, SecretKey};
// sha3 用于生成地址
use hex;
use sha3::{Digest, Keccak256};
// sqlx 用于数据库操作
use sqlx::{MySql, Pool, Row, mysql::MySqlPoolOptions};
// aes_gcm 用于数据加密
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
// base64 用于编码
use base64::{Engine as _, engine::general_purpose};
// rand 用于生成随机数
use rand::RngCore;

// dotenvy 用于加载环境变量
use dotenvy::dotenv;
use std::{env, sync::Arc};
// ... existing code ...

/// 应用状态结构体
/// 包含数据库连接池、加密密钥和环境标识
#[derive(Clone)]
struct AppState {
    db: Pool<MySql>,
    enc_key: [u8; 32],
    is_dev: bool,
}

// ==== 生成钱包逻辑 ====
/// 生成一个新的钱包，包括助记词、私钥和地址
fn generate_wallet() -> (String, String, String) {
    // 生成12个单词的英文助记词
    let mnemonic = Mnemonic::generate_in(Language::English, 12).unwrap();
    // 获取助记词短语
    let phrase = mnemonic.to_string();

    // 从助记词生成种子
    let seed = mnemonic.to_seed("");
    // 创建secp256k1上下文
    let secp = Secp256k1::new();
    // 从种子的前32字节创建私钥
    let secret_key = SecretKey::from_byte_array(seed[..32].try_into().unwrap());
    // 从私钥生成公钥
    let public_key = PublicKey::from_secret_key(&secp, &secret_key.unwrap());

    // 序列化未压缩的公钥并去掉第一个字节
    let public_key_bytes = &public_key.serialize_uncompressed()[1..];
    // 对公钥进行Keccak256哈希
    let hash = Keccak256::digest(public_key_bytes);
    // 取哈希结果的后20字节作为地址，并添加0x前缀
    let address = format!("0x{}", hex::encode(&hash[12..]));

    // 返回助记词、私钥（十六进制格式）和地址
    (
        phrase,
        hex::encode(secret_key.unwrap().secret_bytes()),
        address,
    )
}
// ==== AES 加解密 ====
/// 使用AES-GCM算法加密数据
fn encrypt_data(key: &[u8; 32], plaintext: &str) -> String {
    // 创建AES-GCM加密器
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    // 创建12字节的随机nonce
    let mut nonce_bytes = [0u8; 12];
    rand::rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    // 加密数据
    let ciphertext = cipher.encrypt(nonce, plaintext.as_bytes()).unwrap();
    // 将nonce和密文都进行base64编码并组合返回
    format!(
        "{}:{}",
        general_purpose::STANDARD.encode(nonce_bytes),
        general_purpose::STANDARD.encode(ciphertext)
    )
}

/// 使用AES-GCM算法解密数据
fn decrypt_data(key: &[u8; 32], combined: &str) -> Option<String> {
    // 分割字符串获取nonce和密文
    let parts: Vec<&str> = combined.split(':').collect();
    if parts.len() != 2 {
        return None;
    }
    // 解码nonce和密文
    let nonce_bytes = general_purpose::STANDARD.decode(parts[0]).ok()?;
    let ciphertext_bytes = general_purpose::STANDARD.decode(parts[1]).ok()?;
    // 创建解密器
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Nonce::from_slice(&nonce_bytes);
    // 解密数据
    let plaintext = cipher.decrypt(nonce, ciphertext_bytes.as_ref()).ok()?;
    // 返回解密后的字符串
    Some(String::from_utf8_lossy(&plaintext).to_string())
}

// ==== 数据库写入 ====
/// 将钱包信息保存到数据库
async fn save_wallet(
    db: &Pool<MySql>,
    address: &str,
    mnemonic_enc: &str,
    private_key_enc: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO wallets (address, mnemonic_enc, private_key_enc) VALUES (?, ?, ?)",
        address,
        mnemonic_enc,
        private_key_enc
    )
    .execute(db)
    .await?;
    Ok(())
}


// ==== API 响应结构 ====
/// 创建钱包的响应结构
#[derive(Serialize)]
struct WalletResponse {
    address: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    mnemonic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    private_key: Option<String>,
}

/// 钱包列表响应结构
#[derive(Serialize)]
struct WalletListResponse {
    wallets: Vec<String>,
    count: usize,
}

/// 钱包详情响应结构
#[derive(Serialize)]
struct WalletDetailResponse {
    address: String,
    created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    mnemonic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    private_key: Option<String>,
}

// ==== 路由实现 ====
/// 创建新钱包的处理函数
async fn create_wallet(State(app): State<Arc<AppState>>) -> Json<WalletResponse> {
    // 生成钱包信息
    let (mnemonic, private_key, address) = generate_wallet();
    // 加密助记词和私钥
    let enc_mnemonic = encrypt_data(&app.enc_key, &mnemonic);
    let enc_private = encrypt_data(&app.enc_key, &private_key);

    // 保存到数据库
    if let Err(e) = save_wallet(&app.db, &address, &enc_mnemonic, &enc_private).await {
        eprintln!("保存数据库失败: {}", e);
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


// 获取全部钱包地址列表
async fn list_wallets(State(app): State<Arc<AppState>>) -> Json<WalletListResponse> {
    // 从数据库查询最多50个钱包地址，按ID倒序排列
    let records = sqlx::query("SELECT address FROM wallets ORDER BY id DESC LIMIT 50")
        .fetch_all(&app.db)
        .await
        .unwrap_or_default();

    // 提取地址列表
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


// 获取指定钱包详情
async fn get_wallet(
    State(app): State<Arc<AppState>>,
    Path(address): Path<String>,
) -> Json<WalletDetailResponse> {
    // 从数据库查询指定地址的钱包
    let record_result = sqlx::query(
        "SELECT address, mnemonic_enc, private_key_enc, created_at FROM wallets WHERE address = ?",
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

            // 如果是开发环境，解密助记词和私钥
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
            eprintln!("数据库错误: {}", e);
            Json(WalletDetailResponse {
                address: address.to_string(),
                created_at: "查询失败".to_string(),
                mnemonic: None,
                private_key: None,
            })
        }
    }
}

/// 主函数，程序入口点
#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // 加载.env文件中的环境变量
    dotenv().ok();

    // 从环境变量获取数据库URL、加密密钥和环境类型
    let db_url = env::var("DATABASE_URL")?;
    let enc_key = env::var("ENCRYPTION_KEY")?;
    let env_type = env::var("ENVIRONMENT").unwrap_or_else(|_| "prod".to_string());
    let is_dev = env_type == "dev";

    // 将加密密钥转换为32字节的数组
    let key_bytes = <[u8; 32]>::try_from(enc_key.as_bytes()).expect("ENCRYPTION_KEY 必须是32字节");

    // 创建MySQL数据库连接池
    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await?;

    // 创建应用状态
    let app_state = Arc::new(AppState {
        db: pool,
        enc_key: key_bytes,
        is_dev,
    });

    // 创建路由
    let app = Router::new()
        .route("/wallets", post(create_wallet).get(list_wallets))
        .route("/wallets/{address}", get(get_wallet))
        .with_state(app_state);

    // 打印启动信息
    println!("🚀 服务器启动在 http://127.0.0.1:3000");
    // 使用 TcpListener 替代已废弃的 Server::bind
    let listener = TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;

    Ok(())
}
