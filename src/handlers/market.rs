// ============================================
// 市场数据和 WebSocket 处理器
// ============================================

use axum::{Json, extract::{Path, State}};
use std::sync::Arc;
use crate::config::AppState;
use crate::models::CryptoPrice;
use rand::Rng;

/// 💹 获取实时加密货币价格（使用 CoinGecko API）
pub async fn get_crypto_prices(
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

/// 📈 获取市场趋势（前 10 大币种）
pub async fn get_market_trends(
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

/// 💼 计算钱包组合价值（多链资产汇总）
pub async fn calculate_portfolio_value(
    State(_app): State<Arc<AppState>>,
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
    use sha3::{Digest, Keccak256};
    
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

/// ⏰ 价格提醒设置（保存到数据库）
pub async fn set_price_alert(
    State(_app): State<Arc<AppState>>,
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
