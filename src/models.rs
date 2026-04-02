// ============================================
// 数据模型模块
// ============================================

use serde::{Serialize, Deserialize};

// ============================================
// 🔐 用户认证相关结构体
// ============================================

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: i32,           // 用户 ID
    pub username: String,
    pub exp: usize,         // 过期时间
    pub iat: usize,         // 签发时间
}

// ============================================
// 💼 钱包相关结构体
// ============================================

#[derive(Serialize)]
pub struct WalletResponse {
    pub address: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mnemonic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private_key: Option<String>,
}

#[derive(Serialize)]
pub struct WalletListResponse {
    pub wallets: Vec<String>,
    pub count: usize,
}

#[derive(Serialize)]
pub struct WalletDetailResponse {
    pub address: String,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mnemonic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private_key: Option<String>,
}

#[derive(Serialize)]
pub struct BatchCreateResponse {
    pub message: String,
    pub wallets: Vec<WalletInfo>,
    pub count: usize,
}

#[derive(Serialize, Clone)]
pub struct WalletInfo {
    pub address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mnemonic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private_key: Option<String>,
}

#[derive(Deserialize)]
pub struct ImportWalletRequest {
    pub mnemonic: String,
}

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
}

#[derive(Deserialize)]
pub struct BatchDeleteRequest {
    pub addresses: Vec<String>,
}

// ============================================
// 📊 统计和健康检查结构体
// ============================================

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub database: String,
    pub timestamp: String,
    pub version: String,
}

#[derive(Serialize)]
pub struct StatsResponse {
    pub total_wallets: usize,
    pub created_today: usize,
    pub created_this_week: usize,
    pub avg_per_day: f64,
    pub first_wallet_date: Option<String>,
    pub last_wallet_date: Option<String>,
}

#[derive(Serialize)]
pub struct RecentWallet {
    pub address: String,
    pub created_at: String,
}

#[derive(Serialize)]
pub struct ActivityResponse {
    pub recent: Vec<RecentWallet>,
    pub limit: usize,
}

#[derive(Deserialize)]
pub struct ValidateAddressRequest {
    pub address: String,
}

#[derive(Serialize)]
pub struct ValidateResponse {
    pub valid: bool,
    pub message: String,
}

#[derive(Serialize)]
pub struct MnemonicOnlyResponse {
    pub mnemonic: String,
    pub message: String,
}

#[derive(Deserialize)]
pub struct PrivateKeyRequest {
    pub private_key: String,
}

// ============================================
// 🎯 标签系统结构体
// ============================================

#[derive(Serialize, Deserialize)]
pub struct TagRequest {
    pub tags: Vec<String>,
}

// ============================================
// 💰 余额和交易模拟结构体
// ============================================

#[derive(Serialize, Deserialize)]
pub struct BalanceResponse {
    pub address: String,
    pub eth_balance: String,
    pub usd_value: String,
    pub last_updated: String,
}

#[derive(Serialize, Deserialize)]
pub struct TransactionRecord {
    pub hash: String,
    pub r#type: String,
    pub amount: String,
    pub from: String,
    pub to: String,
    pub timestamp: String,
    pub status: String,
}

// ============================================
// 🎮 娱乐功能结构体
// ============================================

#[derive(Serialize, Deserialize)]
pub struct Achievement {
    pub id: String,
    pub name: String,
    pub description: String,
    pub unlocked: bool,
    pub unlocked_at: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct LuckyDrawResult {
    pub address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mnemonic: Option<String>,
    pub is_lucky: bool,
    pub lucky_factor: String,
    pub message: String,
}

#[derive(Serialize, Deserialize)]
pub struct WalletTheme {
    pub address: String,
    pub color: String,
    pub gradient: String,
    pub emoji: String,
    pub personality: String,
}

#[derive(Serialize, Deserialize)]
pub struct FortuneResponse {
    pub address: String,
    pub date: String,
    pub luck_score: u32,
    pub overall: String,
    pub wealth: String,
    pub advice: String,
    pub lucky_number: u32,
    pub lucky_color: String,
}

// ============================================
// 🔗 区块链工具结构体
// ============================================

#[derive(Serialize, Deserialize)]
pub struct MultiChainWallet {
    pub ethereum: String,
    pub bitcoin: String,
    pub tron: String,
    pub binance_smart_chain: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mnemonic: Option<String>,
}

#[derive(Deserialize)]
pub struct DeriveKeyRequest {
    pub mnemonic: String,
}

#[derive(Deserialize)]
pub struct SignMessageRequest {
    pub private_key: String,
    pub message: String,
}

#[derive(Deserialize)]
pub struct TransferRequest {
    pub from: String,
    pub to: String,
    pub amount: String,
}

// ============================================
// 💰 加密货币价格结构体
// ============================================

#[derive(Serialize, Deserialize)]
pub struct CryptoPrice {
    pub symbol: String,
    pub price_usd: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_btc: Option<f64>,
    pub change_24h: f64,
    pub market_cap: u64,
    pub volume_24h: u64,
    pub last_updated: String,
}
