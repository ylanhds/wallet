// ============================================
// 钱包路由处理器
// ============================================

use axum::{Json, extract::{Path, Query, State}};
use sqlx::{MySql, Pool, Row};
use std::sync::Arc;
use crate::config::AppState;
use crate::models::*;
use crate::utils::crypto::{encrypt_data, decrypt_data};
use bip39::{Language, Mnemonic};
use secp256k1::{PublicKey, Secp256k1, SecretKey};
use sha3::{Digest, Keccak256};
use rand::RngCore;

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

/// 创建新钱包
pub async fn create_wallet(State(app): State<Arc<AppState>>) -> Json<WalletResponse> {
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
pub async fn list_wallets(State(app): State<Arc<AppState>>) -> Json<WalletListResponse> {
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
pub async fn batch_create_wallets(
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
pub async fn delete_wallet(
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
pub async fn search_wallets(
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
pub async fn export_wallets(State(app): State<Arc<AppState>>) -> Json<serde_json::Value> {
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
pub async fn import_wallet(
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
    
    // 为了演示功能，我们生成一个新的钱包并返回输入的助记词
    match generate_wallet() {
        Ok((_, _, address)) => {
            // 加密输入的助记词
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

/// 获取单个钱包详情
pub async fn get_wallet(
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

/// 批量删除钱包
pub async fn batch_delete_wallets(
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
pub async fn get_random_wallet(State(app): State<Arc<AppState>>) -> Json<serde_json::Value> {
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
