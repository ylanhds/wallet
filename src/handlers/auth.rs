// ============================================
// 认证路由处理器
// ============================================

use axum::{Json, extract::State};
use std::sync::Arc;
use crate::config::AppState;
use crate::models::{RegisterRequest, LoginRequest, Claims};
use jsonwebtoken::{encode, decode, Header, Validation, EncodingKey, DecodingKey};
use chrono::{Duration as ChronoDuration, Utc};

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
pub async fn register(
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
pub async fn login(
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
                let _ = sqlx::query("UPDATE ws_users SET last_login = NOW() WHERE id = ?")
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
pub async fn get_current_user(
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
