// ============================================
// WebSocket 实时价格推送模块
// ============================================

use axum::extract::ws::{WebSocket, Message as WsMessage, Utf8Bytes};
use axum::extract::State;
use futures_util::{stream::StreamExt, SinkExt};
use std::sync::Arc;
use crate::config::AppState;
use tokio_tungstenite::{connect_async, tungstenite::Message};

/// WebSocket 处理器（客户端连接到此端点）
pub async fn ws_handler(
    ws: axum::extract::WebSocketUpgrade,
    State(app): axum::extract::State<Arc<AppState>>,
) -> impl axum::response::IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, app))
}

/// 处理单个 WebSocket 连接
async fn handle_socket(
    socket: WebSocket,
    app: Arc<AppState>,
) {
    let (mut sender, mut receiver) = socket.split();
    
    // 订阅广播通道
    let mut rx = app.ws_tx.subscribe();
    
    // 发送任务：接收广播消息并发送给客户端
    let send_task = async move {
        while let Ok(msg) = rx.recv().await {
            if sender.send(WsMessage::Text(Utf8Bytes::from(msg))).await.is_err() {
                break;
            }
        }
    };
    
    // 接收任务：处理客户端消息（心跳等）
    let recv_task = async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let WsMessage::Close(_) = msg {
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
pub async fn connect_to_cryptocompare(app: Arc<AppState>) {
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
                                        let _ = app.ws_tx.send(price_data.to_string());
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
