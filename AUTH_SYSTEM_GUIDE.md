# 🔐 用户认证系统使用指南

## 📋 功能概述

已实现的用户认证功能包括：
- ✅ 用户注册（返回 JWT 令牌）
- ✅ 用户登录（密码 bcrypt 加密验证）
- ✅ JWT 令牌验证
- ✅ 获取当前用户信息
- ✅ 数据库表自动迁移

---

## 🚀 第一步：运行数据库迁移

### 方法一：使用安全迁移脚本（推荐）

```bash
# 1. 编辑 migration_safe.sql，替换数据库名
# 将第一行的 your_database_name 改为你的数据库名

# 2. 执行迁移
mysql -u dev -p < database/migration_safe.sql
# 输入密码：dev9856AC
```

### 方法二：手动执行 SQL

```sql
-- 在 MySQL 中执行
USE your_database_name;

-- 创建用户表
CREATE TABLE IF NOT EXISTS users (
    id INT AUTO_INCREMENT PRIMARY KEY,
    username VARCHAR(50) UNIQUE NOT NULL,
    email VARCHAR(100) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    avatar_url VARCHAR(255),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    last_login TIMESTAMP NULL,
    is_active BOOLEAN DEFAULT TRUE,
    INDEX idx_username (username),
    INDEX idx_email (email)
);

-- 创建会话表
CREATE TABLE IF NOT EXISTS user_sessions (
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

-- 修改 wallets 表添加 user_id
ALTER TABLE wallets ADD COLUMN user_id INT AFTER id;
ALTER TABLE wallets ADD CONSTRAINT fk_wallets_user FOREIGN KEY (user_id) REFERENCES users(id);
CREATE INDEX idx_user_id ON wallets(user_id);
```

### 验证迁移结果

```sql
-- 查看所有表
SHOW TABLES;

-- 应该看到：users, user_sessions, wallets

-- 查看表结构
DESCRIBE users;
DESCRIBE user_sessions;
DESCRIBE wallets;
```

---

## ⚙️ 第二步：配置环境变量

`.env` 文件已自动添加 `JWT_SECRET`：

```env
DATABASE_URL=mysql://dev:dev9856AC@192.168.0.23:3306/zbs
ENCRYPTION_KEY=12345678901234567890123456789012
ENVIRONMENT=dev
JWT_SECRET=your-super-secret-jwt-key-change-in-production  # 🔐 新增
```

**⚠️ 安全提示**：生产环境请更换 `JWT_SECRET`！

---

## 📦 第三步：编译和运行

```bash
# 编译项目（会自动下载新依赖）
cargo build

# 或使用 dx
dx serve
```

### 新增依赖说明
```toml
jsonwebtoken = "9"      # JWT 令牌生成和验证
bcrypt = "0.15"         # 密码加密哈希
uuid = { version = "1", features = ["v4"] }  # UUID 生成
```

---

## 🎯 API 使用示例

### 1. 用户注册

**请求：**
```http
POST http://127.0.0.1:3000/auth/register
Content-Type: application/json

{
    "username": "testuser",
    "email": "test@example.com",
    "password": "123456"
}
```

**响应：**
```json
{
    "success": true,
    "message": "注册成功",
    "user": {
        "id": 1,
        "username": "testuser",
        "email": "test@example.com"
    },
    "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
    "refresh_token": "a1b2c3d4-e5f6-7890-abcd-ef1234567890"
}
```

**说明：**
- `token`: JWT 访问令牌，有效期 24 小时
- `refresh_token`: 刷新令牌（暂未实现刷新功能）

---

### 2. 用户登录

**请求：**
```http
POST http://127.0.0.1:3000/auth/login
Content-Type: application/json

{
    "username": "testuser",
    "password": "123456"
}
```

**响应：**
```json
{
    "success": true,
    "message": "登录成功",
    "user": {
        "id": 1,
        "username": "testuser",
        "email": "",
        "avatar_url": null
    },
    "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
    "refresh_token": "b2c3d4e5-f6a7-8901-bcde-f12345678901"
}
```

---

### 3. 获取当前用户信息

**请求：**
```http
GET http://127.0.0.1:3000/auth/me
Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...
```

**响应：**
```json
{
    "success": true,
    "user": {
        "id": 1,
        "username": "testuser",
        "email": "test@example.com",
        "avatar_url": null
    }
}
```

**错误响应（令牌无效）：**
```json
{
    "success": false,
    "message": "无效的令牌"
}
```

---

## 💻 前端集成示例

### JavaScript/TypeScript

```javascript
// 用户注册
async function register(username, email, password) {
    const response = await fetch('http://127.0.0.1:3000/auth/register', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ username, email, password })
    });
    
    const data = await response.json();
    if (data.success) {
        // 保存 token 到 localStorage
        localStorage.setItem('token', data.token);
        return data.user;
    } else {
        throw new Error(data.message);
    }
}

// 用户登录
async function login(username, password) {
    const response = await fetch('http://127.0.0.1:3000/auth/login', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ username, password })
    });
    
    const data = await response.json();
    if (data.success) {
        localStorage.setItem('token', data.token);
        return data.user;
    } else {
        throw new Error(data.message);
    }
}

// 获取当前用户
async function getCurrentUser() {
    const token = localStorage.getItem('token');
    const response = await fetch('http://127.0.0.1:3000/auth/me', {
        headers: {
            'Authorization': `Bearer ${token}`
        }
    });
    
    const data = await response.json();
    if (data.success) {
        return data.user;
    } else {
        throw new Error(data.message);
    }
}

// 使用示例
try {
    await register('张三', 'zhangsan@example.com', '123456');
    const user = await getCurrentUser();
    console.log('当前用户:', user.username);
} catch (error) {
    console.error('错误:', error.message);
}
```

---

## 🔒 安全性说明

### 密码加密
- 使用 bcrypt 算法进行哈希
- cost factor = 12（安全与性能的平衡）
- 即使数据库泄露，密码也难以破解

### JWT 令牌
- 有效期 24 小时
- 包含用户 ID 和用户名
- 使用 HS256 算法签名
- 每次请求验证签名

### 最佳实践
1. **生产环境**：更换 `JWT_SECRET` 为强随机字符串
2. **HTTPS**：生产环境必须使用 HTTPS 传输
3. **令牌存储**：前端建议使用 HttpOnly Cookie
4. **定期轮换**：定期更换 JWT_SECRET

---

## 📊 数据库表结构

### users 表
```sql
+---------------+--------------+------+-----+-------------------+
| Field         | Type         | Null | Key | Default           |
+---------------+--------------+------+-----+-------------------+
| id            | int          | NO   | PRI | NULL              |
| username      | varchar(50)  | NO   | UNI | NULL              |
| email         | varchar(100) | NO   | UNI | NULL              |
| password_hash | varchar(255) | NO   |     | NULL              |
| avatar_url    | varchar(255) | YES  |     | NULL              |
| created_at    | timestamp    | YES  |     | CURRENT_TIMESTAMP |
| updated_at    | timestamp    | YES  |     | CURRENT_TIMESTAMP |
| last_login    | timestamp    | YES  |     | NULL              |
| is_active     | tinyint(1)   | YES  |     | 1                 |
+---------------+--------------+------+-----+-------------------+
```

### user_sessions 表
```sql
+---------------+--------------+------+-----+-------------------+
| Field         | Type         | Null | Key | Default           |
+---------------+--------------+------+-----+-------------------+
| id            | int          | NO   | PRI | NULL              |
| user_id       | int          | NO   | MUL | NULL              |
| token         | varchar(255) | NO   | UNI | NULL              |
| refresh_token | varchar(255) | YES  |     | NULL              |
| expires_at    | timestamp    | NO   | MUL | NULL              |
| ip_address    | varchar(45)  | YES  |     | NULL              |
| user_agent    | text         | YES  |     | NULL              |
| created_at    | timestamp    | YES  |     | CURRENT_TIMESTAMP |
+---------------+--------------+------+-----+-------------------+
```

### wallets 表（已修改）
```sql
+-----------------+--------------+------+-----+-------------------+
| Field           | Type         | Null | Key | Default           |
+-----------------+--------------+------+-----+-------------------+
| id              | int          | NO   | PRI | NULL              |
| user_id         | int          | YES  | MUL | NULL              | ← 新增
| address         | varchar(255) | NO   | UNI | NULL              |
| mnemonic_enc    | text         | NO   |     | NULL              |
| private_key_enc | text         | NO   |     | NULL              |
| label           | varchar(100) | YES  |     | NULL              | ← 新增
| tags            | json         | YES  |     | NULL              | ← 新增
| created_at      | timestamp    | YES  |     | CURRENT_TIMESTAMP |
+-----------------+--------------+------+-----+-------------------+
```

---

## 🧪 测试用例

### 1. 测试注册流程

```bash
# 注册新用户
curl -X POST http://127.0.0.1:3000/auth/register \
  -H "Content-Type: application/json" \
  -d '{
    "username": "alice",
    "email": "alice@example.com",
    "password": "password123"
  }'

# 输出：
# {"success":true,"message":"注册成功","user":{"id":1,"username":"alice","email":"alice@example.com"},"token":"...","refresh_token":"..."}
```

### 2. 测试登录流程

```bash
# 登录
curl -X POST http://127.0.0.1:3000/auth/login \
  -H "Content-Type: application/json" \
  -d '{
    "username": "alice",
    "password": "password123"
  }'

# 输出：
# {"success":true,"message":"登录成功","user":{"id":1,"username":"alice","email":"","avatar_url":null},"token":"...","refresh_token":"..."}
```

### 3. 测试令牌验证

```bash
# 使用正确的令牌
TOKEN="eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."
curl -X GET http://127.0.0.1:3000/auth/me \
  -H "Authorization: Bearer $TOKEN"

# 输出：
# {"success":true,"user":{"id":1,"username":"alice","email":"alice@example.com","avatar_url":null}}
```

### 4. 测试错误场景

```bash
# 错误的密码
curl -X POST http://127.0.0.1:3000/auth/login \
  -H "Content-Type: application/json" \
  -d '{
    "username": "alice",
    "password": "wrongpassword"
  }'

# 输出：
# {"success":false,"message":"用户名或密码错误"}

# 无效的令牌
curl -X GET http://127.0.0.1:3000/auth/me \
  -H "Authorization: Bearer invalid_token"

# 输出：
# {"success":false,"message":"无效的令牌"}
```

---

## 🔄 下一步计划

### 第二阶段：价格提醒系统
- [ ] 创建 `price_alerts` 表
- [ ] 实现设置提醒 API
- [ ] 后台定时检查任务
- [ ] 触发通知推送

### 第三阶段：通知系统
- [ ] 创建 `notifications` 表
- [ ] 站内通知功能
- [ ] 邮件通知集成

---

## ❓ 常见问题

### Q: 为什么需要 JWT？
A: JWT 是无状态认证的标准方案，适合 RESTful API 和微服务架构。

### Q: 令牌过期了怎么办？
A: 目前令牌有效期 24 小时。后续会实现 refresh_token 刷新机制。

### Q: 如何保护敏感 API？
A: 在路由前添加中间件验证 JWT，例如：
```rust
async fn auth_middleware(headers: HeaderMap, app: State<Arc<AppState>>) {
    // 验证 token
}
```

### Q: 支持第三方登录吗？
A: 目前只支持用户名密码登录。后续可以添加 GitHub、Google 等 OAuth 登录。

---

## 📝 更新日志

### v0.1.0 - 2024-01-01
- ✅ 添加用户注册功能
- ✅ 添加用户登录功能
- ✅ 实现 JWT 认证
- ✅ 创建数据库表
- ✅ 添加 bcrypt 密码加密

---

**祝你使用愉快！** 🎉

如有问题，请查看主文档 [FEATURE_SUGGESTIONS.md](FEATURE_SUGGESTIONS.md)
