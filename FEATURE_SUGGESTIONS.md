# 🚀 项目功能扩展建议

基于当前钱包服务项目，以下是可以添加的新功能和数据库表设计。

---

## 📊 当前功能概览

### ✅ 已有功能
- ✅ 钱包管理（创建/删除/导入/导出/批量操作）
- ✅ 实时价格监控（WebSocket + CryptoCompare）
- ✅ 娱乐功能（抽奖/成就/运势/主题）
- ✅ 区块链工具（多链地址/私钥推导/签名）
- ✅ 标签系统
- ✅ 模拟余额和交易记录

---

## 💡 推荐添加的功能

### 1️⃣ 用户认证系统 🔒

#### 功能描述
添加用户登录注册功能，每个用户可以管理多个钱包。

#### API 端点
```rust
POST   /auth/register       // 用户注册
POST   /auth/login          // 用户登录
POST   /auth/logout         // 用户登出
GET    /auth/me             // 获取当前用户信息
PUT    /auth/password       // 修改密码
POST   /auth/refresh-token  // 刷新令牌
```

#### 数据库表
```sql
-- 用户表
CREATE TABLE users (
    id INT AUTO_INCREMENT PRIMARY KEY,
    username VARCHAR(50) UNIQUE NOT NULL,
    email VARCHAR(100) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    avatar_url VARCHAR(255),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    last_login TIMESTAMP,
    is_active BOOLEAN DEFAULT TRUE,
    INDEX idx_username (username),
    INDEX idx_email (email)
);

-- 用户会话表（用于 Token 管理）
CREATE TABLE user_sessions (
    id INT AUTO_INCREMENT PRIMARY KEY,
    user_id INT NOT NULL,
    token VARCHAR(255) UNIQUE NOT NULL,
    refresh_token VARCHAR(255),
    expires_at TIMESTAMP NOT NULL,
    ip_address VARCHAR(45),
    user_agent TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    INDEX idx_token (token),
    INDEX idx_expires (expires_at)
);

-- 修改现有钱包表，添加 user_id
ALTER TABLE wallets ADD COLUMN user_id INT AFTER id;
ALTER TABLE wallets ADD FOREIGN KEY (user_id) REFERENCES users(id);
ALTER TABLE wallets ADD INDEX idx_user_id (user_id);
```

#### 技术实现
- JWT (JSON Web Token) 认证
- bcrypt 密码哈希
- 刷新令牌机制

---

### 2️⃣ 价格提醒系统 ⏰

#### 功能描述
用户可以设置价格提醒，当币种价格达到目标值时收到通知。

#### API 端点
```rust
POST   /alerts/price              // 创建价格提醒
GET    /alerts/price              // 获取我的提醒列表
DELETE /alerts/price/:id          // 删除提醒
GET    /alerts/history            // 提醒触发历史
PUT    /alerts/price/:id/toggle   // 启用/禁用提醒
```

#### 数据库表
```sql
-- 价格提醒表
CREATE TABLE price_alerts (
    id INT AUTO_INCREMENT PRIMARY KEY,
    user_id INT NOT NULL,
    symbol VARCHAR(10) NOT NULL,  -- BTC, ETH, etc.
    target_price DECIMAL(20, 8) NOT NULL,
    condition ENUM('above', 'below') NOT NULL,
    is_active BOOLEAN DEFAULT TRUE,
    triggered_at TIMESTAMP NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    INDEX idx_user_symbol (user_id, symbol),
    INDEX idx_active (is_active)
);

-- 提醒触发历史
CREATE TABLE alert_history (
    id INT AUTO_INCREMENT PRIMARY KEY,
    alert_id INT NOT NULL,
    triggered_price DECIMAL(20, 8) NOT NULL,
    triggered_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    notification_sent BOOLEAN DEFAULT FALSE,
    FOREIGN KEY (alert_id) REFERENCES price_alerts(id) ON DELETE CASCADE,
    INDEX idx_triggered (triggered_at),
    INDEX idx_sent (notification_sent)
);
```

#### 后台任务
```rust
// 每分钟检查一次价格提醒
async fn check_price_alerts() {
    loop {
        // 1. 获取所有活跃的价格提醒
        // 2. 获取当前价格
        // 3. 比较并触发提醒
        // 4. 记录到 alert_history
        // 5. 发送通知（邮件/站内信）
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}
```

---

### 3️⃣ 投资组合跟踪 📈

#### 功能描述
用户可以记录自己的持仓，查看投资收益和盈亏分析。

#### API 端点
```rust
POST   /portfolio/holdings          // 添加持仓
GET    /portfolio/holdings          // 获取持仓列表
PUT    /portfolio/holdings/:id      // 更新持仓
DELETE /portfolio/holdings/:id      // 删除持仓
GET    /portfolio/performance       // 收益分析
GET    /portfolio/allocation        // 资产配置饼图
```

#### 数据库表
```sql
-- 持仓表
CREATE TABLE portfolio_holdings (
    id INT AUTO_INCREMENT PRIMARY KEY,
    user_id INT NOT NULL,
    symbol VARCHAR(10) NOT NULL,
    amount DECIMAL(30, 18) NOT NULL,
    avg_buy_price DECIMAL(20, 8) NOT NULL,
    current_price DECIMAL(20, 8),
    purchase_date DATE,
    notes TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    INDEX idx_user_symbol (user_id, symbol),
    INDEX idx_symbol (symbol)
);

-- 交易记录表
CREATE TABLE portfolio_transactions (
    id INT AUTO_INCREMENT PRIMARY KEY,
    user_id INT NOT NULL,
    symbol VARCHAR(10) NOT NULL,
    type ENUM('buy', 'sell') NOT NULL,
    amount DECIMAL(30, 18) NOT NULL,
    price DECIMAL(20, 8) NOT NULL,
    total_value DECIMAL(20, 8) NOT NULL,
    fee DECIMAL(20, 8) DEFAULT 0,
    transaction_date TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    notes TEXT,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    INDEX idx_user_symbol (user_id, symbol),
    INDEX idx_date (transaction_date),
    INDEX idx_type (type)
);

-- 投资组合快照（每日记录）
CREATE TABLE portfolio_snapshots (
    id INT AUTO_INCREMENT PRIMARY KEY,
    user_id INT NOT NULL,
    snapshot_date DATE NOT NULL,
    total_value DECIMAL(20, 8) NOT NULL,
    total_cost DECIMAL(20, 8) NOT NULL,
    profit_loss DECIMAL(20, 8) NOT NULL,
    profit_loss_percent DECIMAL(10, 4) NOT NULL,
    top_gainer VARCHAR(10),
    top_loser VARCHAR(10),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    UNIQUE KEY unique_user_date (user_id, snapshot_date),
    INDEX idx_date (snapshot_date)
);
```

#### 分析功能
```rust
// 计算收益率
struct PerformanceMetrics {
    total_invested: Decimal,
    current_value: Decimal,
    total_profit: Decimal,
    profit_percent: Decimal,
    best_performer: String,
    worst_performer: String,
}

// 生成收益曲线数据
async fn get_performance_chart(user_id: i32, days: u32) -> Vec<Snapshot> {
    // 查询 portfolio_snapshots
    // 返回指定天数的数据点
}
```

---

### 4️⃣ 新闻资讯聚合 📰

#### 功能描述
聚合加密货币相关新闻，用户可以查看最新资讯。

#### API 端点
```rust
GET /news              // 获取新闻列表
GET /news/:id          // 新闻详情
GET /news/sentiment    // 市场情绪分析
```

#### 数据库表
```sql
-- 新闻表
CREATE TABLE news (
    id INT AUTO_INCREMENT PRIMARY KEY,
    title VARCHAR(255) NOT NULL,
    summary TEXT,
    content TEXT,
    source VARCHAR(100),
    author VARCHAR(100),
    url VARCHAR(500) UNIQUE NOT NULL,
    image_url VARCHAR(500),
    published_at TIMESTAMP,
    sentiment_score DECIMAL(5, 4),  -- -1 到 1，负面到正面
    related_symbols JSON,  -- ["BTC", "ETH"]
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_published (published_at),
    INDEX idx_sentiment (sentiment_score),
    INDEX idx_symbols ((CAST(related_symbols AS CHAR(100))))
);

-- 用户阅读历史
CREATE TABLE news_read_history (
    id INT AUTO_INCREMENT PRIMARY KEY,
    user_id INT NOT NULL,
    news_id INT NOT NULL,
    read_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    read_duration_seconds INT,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (news_id) REFERENCES news(id) ON DELETE CASCADE,
    UNIQUE KEY unique_user_news (user_id, news_id),
    INDEX idx_read_at (read_at)
);
```

#### 数据源
- CoinDesk API
- CryptoPanic API
- Twitter API（KOL 动态）
- Reddit API

---

### 5️⃣ 交易所集成 💱

#### 功能描述
连接主流交易所 API，支持查看行情和模拟交易。

#### API 端点
```rust
GET  /exchange/tickers         // 获取交易所行情
GET  /exchange/:name/kline     // K 线数据
POST /exchange/order/simulate  // 模拟下单
GET  /exchange/orders          // 订单历史
```

#### 数据库表
```sql
-- 支持的交易所
CREATE TABLE exchanges (
    id INT AUTO_INCREMENT PRIMARY KEY,
    name VARCHAR(50) UNIQUE NOT NULL,  -- binance, okx, bybit
    display_name VARCHAR(100),
    api_base_url VARCHAR(255),
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- K 线数据缓存
CREATE TABLE kline_data (
    id INT AUTO_INCREMENT PRIMARY KEY,
    exchange_id INT NOT NULL,
    symbol VARCHAR(20) NOT NULL,  -- BTCUSDT
    interval VARCHAR(10) NOT NULL,  -- 1m, 5m, 1h, 1d
    open_time TIMESTAMP NOT NULL,
    close_time TIMESTAMP NOT NULL,
    open_price DECIMAL(30, 8) NOT NULL,
    high_price DECIMAL(30, 8) NOT NULL,
    low_price DECIMAL(30, 8) NOT NULL,
    close_price DECIMAL(30, 8) NOT NULL,
    volume DECIMAL(30, 8) NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (exchange_id) REFERENCES exchanges(id),
    UNIQUE KEY unique_kline (exchange_id, symbol, interval, open_time),
    INDEX idx_symbol_time (symbol, open_time)
);

-- 模拟订单
CREATE TABLE simulated_orders (
    id INT AUTO_INCREMENT PRIMARY KEY,
    user_id INT NOT NULL,
    exchange_id INT NOT NULL,
    symbol VARCHAR(20) NOT NULL,
    side ENUM('buy', 'sell') NOT NULL,
    type ENUM('market', 'limit') NOT NULL,
    amount DECIMAL(30, 8) NOT NULL,
    price DECIMAL(30, 8),
    status ENUM('pending', 'filled', 'cancelled') DEFAULT 'pending',
    filled_amount DECIMAL(30, 8) DEFAULT 0,
    avg_fill_price DECIMAL(30, 8),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id),
    FOREIGN KEY (exchange_id) REFERENCES exchanges(id),
    INDEX idx_user_status (user_id, status),
    INDEX idx_created (created_at)
);
```

---

### 6️⃣ 社交功能 👥

#### 功能描述
用户可以关注其他交易者，查看他们的投资组合（可选公开）。

#### API 端点
```rust
GET    /users/:id/profile       // 用户主页
GET    /users/:id/portfolios    // 公开的投资组合
POST   /users/:id/follow        // 关注用户
DELETE /users/:id/follow        // 取消关注
GET    /users/me/followers      // 我的粉丝
GET    /users/me/following      // 我关注的
GET    /feed                    // 动态Feed流
```

#### 数据库表
```sql
-- 关注关系
CREATE TABLE follows (
    id INT AUTO_INCREMENT PRIMARY KEY,
    follower_id INT NOT NULL,      -- 关注者
    following_id INT NOT NULL,     -- 被关注者
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (follower_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (following_id) REFERENCES users(id) ON DELETE CASCADE,
    UNIQUE KEY unique_follow (follower_id, following_id),
    INDEX idx_follower (follower_id),
    INDEX idx_following (following_id)
);

-- 用户动态
CREATE TABLE user_activities (
    id INT AUTO_INCREMENT PRIMARY KEY,
    user_id INT NOT NULL,
    activity_type ENUM('create_wallet', 'achievement', 'trade', 'post') NOT NULL,
    content JSON,  -- 动态内容
    visibility ENUM('public', 'followers', 'private') DEFAULT 'public',
    likes_count INT DEFAULT 0,
    comments_count INT DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    INDEX idx_user_time (user_id, created_at),
    INDEX idx_visibility (visibility)
);

-- 点赞
CREATE TABLE activity_likes (
    id INT AUTO_INCREMENT PRIMARY KEY,
    user_id INT NOT NULL,
    activity_id INT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (activity_id) REFERENCES user_activities(id) ON DELETE CASCADE,
    UNIQUE KEY unique_like (user_id, activity_id)
);

-- 评论
CREATE TABLE activity_comments (
    id INT AUTO_INCREMENT PRIMARY KEY,
    user_id INT NOT NULL,
    activity_id INT NOT NULL,
    parent_id INT NULL,  -- 支持回复评论
    content TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (activity_id) REFERENCES user_activities(id) ON DELETE CASCADE,
    FOREIGN KEY (parent_id) REFERENCES activity_comments(id) ON DELETE CASCADE,
    INDEX idx_activity (activity_id, created_at)
);
```

---

### 7️⃣ 数据可视化大屏 📊

#### 功能描述
提供数据可视化大屏，展示市场总览、用户统计等。

#### API 端点
```rust
GET /dashboard/overview      // 市场总览
GET /dashboard/statistics    // 平台统计
GET /dashboard/heatmap       // 涨跌热力图
```

#### 数据库表
```sql
-- 平台统计数据（每小时更新）
CREATE TABLE platform_stats (
    id INT AUTO_INCREMENT PRIMARY KEY,
    stat_time TIMESTAMP NOT NULL,
    total_users INT DEFAULT 0,
    total_wallets INT DEFAULT 0,
    active_users_24h INT DEFAULT 0,
    new_wallets_24h INT DEFAULT 0,
    total_api_calls INT DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE KEY unique_stat_time (stat_time),
    INDEX idx_time (stat_time)
);

-- 热门币种统计
CREATE TABLE trending_symbols (
    id INT AUTO_INCREMENT PRIMARY KEY,
    stat_date DATE NOT NULL,
    symbol VARCHAR(10) NOT NULL,
    rank INT NOT NULL,
    view_count INT DEFAULT 0,
    search_count INT DEFAULT 0,
    price_change_24h DECIMAL(10, 4),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE KEY unique_date_symbol (stat_date, symbol),
    INDEX idx_date_rank (stat_date, rank)
);
```

---

### 8️⃣ 通知系统 🔔

#### 功能描述
站内通知、邮件通知、Webhook 推送。

#### API 端点
```rust
GET    /notifications           // 获取我的通知
PUT    /notifications/:id/read  // 标记为已读
DELETE /notifications           // 清空通知
POST   /notifications/test      // 发送测试通知
GET    /notifications/settings  // 通知设置
PUT    /notifications/settings  // 更新设置
```

#### 数据库表
```sql
-- 通知表
CREATE TABLE notifications (
    id INT AUTO_INCREMENT PRIMARY KEY,
    user_id INT NOT NULL,
    type ENUM('price_alert', 'system', 'achievement', 'news') NOT NULL,
    title VARCHAR(255) NOT NULL,
    content TEXT,
    data JSON,  -- 附加数据
    is_read BOOLEAN DEFAULT FALSE,
    read_at TIMESTAMP NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    INDEX idx_user_unread (user_id, is_read),
    INDEX idx_created (created_at)
);

-- 通知设置
CREATE TABLE notification_settings (
    id INT AUTO_INCREMENT PRIMARY KEY,
    user_id INT UNIQUE NOT NULL,
    email_enabled BOOLEAN DEFAULT TRUE,
    push_enabled BOOLEAN DEFAULT TRUE,
    price_alert_enabled BOOLEAN DEFAULT TRUE,
    achievement_enabled BOOLEAN DEFAULT TRUE,
    newsletter_enabled BOOLEAN DEFAULT FALSE,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);
```

---

### 9️⃣ 账本功能 📒

#### 功能描述
记录每笔交易的详细信息，生成财务报表。

#### API 端点
```rust
GET  /ledger/entries           // 获取账本条目
POST /ledger/entries           // 添加条目
GET  /ledger/report/monthly    // 月度报表
GET  /ledger/report/yearly     // 年度报表
```

#### 数据库表
```sql
-- 账本条目
CREATE TABLE ledger_entries (
    id INT AUTO_INCREMENT PRIMARY KEY,
    user_id INT NOT NULL,
    wallet_id INT,
    type ENUM('income', 'expense', 'transfer', 'trade') NOT NULL,
    category VARCHAR(50),  -- 挖矿/空投/交易盈利等
    amount DECIMAL(30, 18) NOT NULL,
    currency VARCHAR(10) NOT NULL,
    usd_value DECIMAL(20, 8),
    description TEXT,
    reference_id VARCHAR(100),  -- 关联的交易 ID
    transaction_date TIMESTAMP NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id),
    FOREIGN KEY (wallet_id) REFERENCES wallets(id),
    INDEX idx_user_type (user_id, type),
    INDEX idx_date (transaction_date),
    INDEX idx_category (category)
);

-- 分类预算
CREATE TABLE budget_categories (
    id INT AUTO_INCREMENT PRIMARY KEY,
    user_id INT NOT NULL,
    category_name VARCHAR(50) NOT NULL,
    monthly_limit DECIMAL(20, 8),
    spent_this_month DECIMAL(20, 8) DEFAULT 0,
    month YEAR_MONTH NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id),
    UNIQUE KEY unique_user_category_month (user_id, category_name, month),
    INDEX idx_month (month)
);
```

---

### 🔟 NFT 收藏管理 🖼️

#### 功能描述
管理用户的 NFT 收藏品，展示 NFT 画廊。

#### API 端点
```rust
GET  /nfts/collection          // 获取 NFT 收藏
GET  /nfts/:address            // 地址的 NFT 列表
POST /nfts/sync                // 同步 NFT 数据
```

#### 数据库表
```sql
-- NFT 藏品
CREATE TABLE nft_collections (
    id INT AUTO_INCREMENT PRIMARY KEY,
    contract_address VARCHAR(42) NOT NULL,
    chain VARCHAR(20) NOT NULL,  -- ethereum, polygon
    name VARCHAR(100) NOT NULL,
    symbol VARCHAR(20),
    floor_price DECIMAL(20, 8),
    total_volume DECIMAL(30, 8),
    logo_url VARCHAR(500),
    description TEXT,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE KEY unique_contract (contract_address, chain),
    INDEX idx_chain (chain)
);

-- NFT 代币
CREATE TABLE nft_tokens (
    id INT AUTO_INCREMENT PRIMARY KEY,
    collection_id INT NOT NULL,
    token_id VARCHAR(100) NOT NULL,
    owner_address VARCHAR(42) NOT NULL,
    metadata JSON,
    image_url VARCHAR(500),
    name VARCHAR(255),
    rarity_rank INT,
    last_sale_price DECIMAL(30, 8),
    acquired_date TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (collection_id) REFERENCES nft_collections(id),
    UNIQUE KEY unique_token (collection_id, token_id),
    INDEX idx_owner (owner_address),
    INDEX idx_rarity (rarity_rank)
);

-- 用户 NFT 关联
CREATE TABLE user_nfts (
    id INT AUTO_INCREMENT PRIMARY KEY,
    user_id INT NOT NULL,
    nft_token_id INT NOT NULL,
    wallet_address VARCHAR(42) NOT NULL,
    is_favorite BOOLEAN DEFAULT FALSE,
    notes TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id),
    FOREIGN KEY (nft_token_id) REFERENCES nft_tokens(id),
    UNIQUE KEY unique_user_nft (user_id, nft_token_id),
    INDEX idx_user (user_id)
);
```

---

## 🎯 优先级建议

### 高优先级（实用性强）⭐⭐⭐
1. **用户认证系统** - 多用户支持的基础
2. **价格提醒系统** - 配合 WebSocket 实时价格
3. **投资组合跟踪** - 核心金融功能
4. **通知系统** - 提升用户体验

### 中优先级（增强体验）⭐⭐
5. **数据可视化大屏** - 展示项目实力
6. **账本功能** - 财务管理工具
7. **新闻资讯聚合** - 增加用户粘性

### 低优先级（锦上添花）⭐
8. **社交功能** - 需要一定用户基础
9. **交易所集成** - 复杂度较高
10. **NFT 收藏管理** - 特定需求

---

## 📋 完整数据库表清单

### 现有表
- `wallets` - 钱包表

### 新增表（按模块）

#### 用户认证模块
- `users` - 用户表
- `user_sessions` - 用户会话表

#### 价格提醒模块
- `price_alerts` - 价格提醒表
- `alert_history` - 提醒历史表

#### 投资组合模块
- `portfolio_holdings` - 持仓表
- `portfolio_transactions` - 交易记录表
- `portfolio_snapshots` - 投资组合快照表

#### 新闻资讯模块
- `news` - 新闻表
- `news_read_history` - 阅读历史表

#### 交易所模块
- `exchanges` - 交易所表
- `kline_data` - K 线数据表
- `simulated_orders` - 模拟订单表

#### 社交模块
- `follows` - 关注关系表
- `user_activities` - 用户动态表
- `activity_likes` - 点赞表
- `activity_comments` - 评论表

#### 数据统计模块
- `platform_stats` - 平台统计表
- `trending_symbols` - 热门币种表

#### 通知模块
- `notifications` - 通知表
- `notification_settings` - 通知设置表

#### 账本模块
- `ledger_entries` - 账本条目表
- `budget_categories` - 分类预算表

#### NFT 模块
- `nft_collections` - NFT 合集表
- `nft_tokens` - NFT 代币表
- `user_nfts` - 用户 NFT 关联表

---

## 💻 实施建议

### 第一阶段：用户系统（1-2 周）
1. 实现用户注册登录
2. JWT 认证中间件
3. 修改现有 API 添加权限控制

### 第二阶段：核心功能（2-3 周）
1. 价格提醒系统
2. 投资组合跟踪
3. 通知系统

### 第三阶段：增值功能（1-2 周）
1. 数据可视化
2. 新闻资讯
3. 账本功能

### 第四阶段：高级功能（按需）
1. 社交功能
2. 交易所集成
3. NFT 管理

---

## 🔧 技术栈推荐

### 认证授权
- `jsonwebtoken` - JWT 实现
- `bcrypt` - 密码加密
- `argon2` - 更安全的密码哈希（可选）

### 邮件发送
- `lettre` - Rust 邮件库
- SMTP 服务（QQ 邮箱、163 邮箱、SendGrid）

### 定时任务
- `tokio-cron-scheduler` - Cron 表达式
- `sqlx` 定时查询

### 数据可视化
- ECharts / Chart.js（前端）
- Rust 后端提供数据接口

---

**祝你开发顺利！** 🚀

*根据你的兴趣和时间选择合适的功能进行开发*
