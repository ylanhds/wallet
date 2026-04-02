// ============================================
// 娱乐功能路由处理器
// ============================================

use axum::{Json, extract::{Path, Query, State}};
use sqlx::Row;
use std::sync::Arc;
use crate::config::AppState;
use crate::models::*;
use crate::utils::crypto::encrypt_data;
use bip39::{Language, Mnemonic};
use secp256k1::{PublicKey, Secp256k1, SecretKey};
use sha3::{Digest, Keccak256};
use rand::Rng;

/// 🎯 给钱包添加标签
pub async fn add_wallet_tags(
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
    
    match sqlx::query("UPDATE ws_wallets SET tags = ? WHERE address = ?")
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
pub async fn get_wallet_tags(
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
pub async fn simulate_balance(
    State(_app): State<Arc<AppState>>,
    Path(address): Path<String>,
) -> Json<BalanceResponse> {
    // 使用地址哈希生成固定的"随机"余额
    let hash = Keccak256::digest(address.as_bytes());
    let balance_int = u32::from_be_bytes([hash[0], hash[1], hash[2], hash[3]]) % 1000;
    let eth_balance = (balance_int as f64) / 100.0;
    let eth_price = 2000.0 + (balance_int % 100) as f64;
    let usd_value = eth_balance * eth_price;
    
    Json(BalanceResponse {
        address,
        eth_balance: format!("{:.4}", eth_balance),
        usd_value: format!("{} USD", usd_value),
        last_updated: chrono::Utc::now().naive_utc().to_string(),
    })
}

/// 📜 生成假的交易记录（仅供娱乐）
pub async fn generate_fake_transactions(
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
pub async fn lucky_draw(State(app): State<Arc<AppState>>) -> Json<LuckyDrawResult> {
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
pub async fn get_achievements(State(app): State<Arc<AppState>>) -> Json<serde_json::Value> {
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
pub async fn clear_all_data(
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

/// 🎨 获取钱包的主题颜色
pub async fn get_wallet_theme(
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
pub async fn get_fortune(
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

// 辅助函数
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
