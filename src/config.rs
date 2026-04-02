// ============================================
// 配置模块
// ============================================

use sqlx::{MySql, Pool};
use tokio::sync::broadcast;

/// 应用状态结构体
#[derive(Clone)]
pub struct AppState {
    pub db: Pool<MySql>,
    pub enc_key: [u8; 32],
    pub is_dev: bool,
    pub ws_tx: broadcast::Sender<String>,  // WebSocket 广播发送器
    pub jwt_secret: String,                 // JWT 密钥
}

impl AppState {
    /// 创建新的 AppState 实例
    pub fn new(
        db: Pool<MySql>,
        enc_key: [u8; 32],
        is_dev: bool,
        jwt_secret: String,
    ) -> (Self, broadcast::Sender<String>) {
        // 创建 WebSocket 广播通道（容量 100 条消息）
        let (ws_tx, _) = broadcast::channel::<String>(100);
        
        let app_state = AppState {
            db,
            enc_key,
            is_dev,
            ws_tx: ws_tx.clone(),
            jwt_secret,
        };
        
        (app_state, ws_tx)
    }
}
