// ============================================
// 钱包服务 - 主入口文件
// 项目：wallet-service (ws_)
// 重构版本：模块化架构
// ============================================

use axum::{Router, routing::{get, post, delete}};
use tokio::net::TcpListener;
use sqlx::{mysql::MySqlPoolOptions};
use dotenvy::dotenv;
use std::{env, sync::Arc};

// 导入模块
mod config;
mod models;
mod utils;
mod handlers;
mod websocket;

use config::AppState;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // 加载环境变量
    dotenv().ok();

    // 读取配置
    let db_url = env::var("DATABASE_URL")?;
    let enc_key = env::var("ENCRYPTION_KEY")?;
    let jwt_secret = env::var("JWT_SECRET").unwrap_or_else(|_| "your-secret-key-change-in-production".to_string());
    let env_type = env::var("ENVIRONMENT").unwrap_or_else(|_| "prod".to_string());
    let is_dev = env_type == "dev";

    // 验证加密密钥长度
    let key_bytes = <[u8; 32]>::try_from(enc_key.as_bytes()).expect("ENCRYPTION_KEY 必须是 32 字节");

    // 创建数据库连接池
    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await?;

    // 创建应用状态
    let (app_state, _ws_tx) = AppState::new(pool, key_bytes, is_dev, jwt_secret);
    let app_state = Arc::new(app_state);

    // 启动 WebSocket 后台任务，连接到 CryptoCompare
    let bg_app_state = app_state.clone();
    tokio::spawn(async move {
        websocket::connect_to_cryptocompare(bg_app_state).await;
    });

    // 构建路由
    let app = Router::new()
        // 🌐 前端页面路由
        .route("/", get(index))
        .route("/auth", get(auth_page))
        .route("/price-monitor", get(price_monitor_page))
        
        // 🔌 WebSocket 端点
        .route("/ws", get(websocket::ws_handler))
        
        // 🏥 健康检查和统计
        .route("/health", get(handlers::common::health_check))
        .route("/stats", get(handlers::common::get_stats))
        .route("/activity", get(handlers::common::get_recent_activity))
        .route("/validate-address", post(handlers::common::validate_address))
        .route("/generate-mnemonic", get(handlers::common::generate_mnemonic_only))
        .route("/address-from-key", post(handlers::common::address_from_private_key))
        
        // 💼 钱包管理核心功能
        .route("/wallets", post(handlers::wallet::create_wallet).get(handlers::wallet::list_wallets))
        .route("/wallets/batch", post(handlers::wallet::batch_create_wallets))
        .route("/wallets/search", get(handlers::wallet::search_wallets))
        .route("/wallets/export", get(handlers::wallet::export_wallets))
        .route("/wallets/import", post(handlers::wallet::import_wallet))
        .route("/wallets/{address}", get(handlers::wallet::get_wallet).delete(handlers::wallet::delete_wallet))
        .route("/wallets/random", get(handlers::wallet::get_random_wallet))
        .route("/wallets/batch-delete", post(handlers::wallet::batch_delete_wallets))
        
        // 🎯 娱乐功能
        .route("/wallets/{address}/tags", post(handlers::entertainment::add_wallet_tags).get(handlers::entertainment::get_wallet_tags))
        .route("/wallets/{address}/balance", get(handlers::entertainment::simulate_balance))
        .route("/wallets/{address}/transactions", get(handlers::entertainment::generate_fake_transactions))
        .route("/lucky-draw", get(handlers::entertainment::lucky_draw))
        .route("/achievements", get(handlers::entertainment::get_achievements))
        .route("/admin/clear-all", delete(handlers::entertainment::clear_all_data))
        .route("/wallets/{address}/theme", get(handlers::entertainment::get_wallet_theme))
        .route("/wallets/{address}/fortune", get(handlers::entertainment::get_fortune))
        
        // 🔗 区块链工具
        .route("/tools/multi-chain", post(handlers::tools::generate_multi_chain_wallet))
        .route("/tools/derive-key", post(handlers::tools::derive_from_private_key))
        .route("/tools/verify-mnemonic", post(handlers::tools::verify_mnemonic))
        
        // 💎 高级区块链功能
        .route("/tools/sign-message", post(handlers::tools::sign_message))
        .route("/tools/verify-signature", post(handlers::tools::verify_signature))
        .route("/tools/simulate-transfer", post(handlers::tools::simulate_transfer))
        .route("/tools/analyze-address/{address}", get(handlers::tools::analyze_address))
        .route("/tools/vanity-address", get(handlers::tools::generate_vanity_address))
        
        // 💰 市场数据
        .route("/market/prices", get(handlers::market::get_crypto_prices))
        .route("/market/trends", get(handlers::market::get_market_trends))
        .route("/portfolio/{address}", get(handlers::market::calculate_portfolio_value))
        .route("/alerts/price", post(handlers::market::set_price_alert))
        
        // 🔐 用户认证
        .route("/auth/register", post(handlers::auth::register))
        .route("/auth/login", post(handlers::auth::login))
        .route("/auth/me", get(handlers::auth::get_current_user))
        
        .with_state(app_state);

    // 启动服务器
    println!("🚀 服务器启动在 http://127.0.0.1:3000");
    println!("✅ 服务已准备就绪，支持中文响应");
    println!("🌐 访问 http://127.0.0.1:3000 查看 Web 界面");
    println!("🎁 新增功能：统计 | 活动 | 验证 | 随机钱包 | 批量删除 | 工具");
    
    let listener = TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;

    Ok(())
}

// ============================================
// 页面路由处理器（直接在 main.rs 中）
// ============================================

/// 主页 HTML
async fn index() -> axum::response::Html<&'static str> {
    axum::response::Html(include_str!("../index.html"))
}

/// 认证页面
async fn auth_page() -> axum::response::Html<&'static str> {
    axum::response::Html(include_str!("../auth.html"))
}

/// 价格监控页面
async fn price_monitor_page() -> axum::response::Html<&'static str> {
    axum::response::Html(include_str!("../price_monitor.html"))
}
