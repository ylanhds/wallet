# 📊 数据库表命名规范

## 🎯 命名原则

为避免与其他测试项目表名冲突，所有本项目表名统一添加 `ws_` 前缀。

---

## 📝 命名规范

### 格式
```
ws_<模块>_名称
```

### 说明
- **`ws`**: wallet-service 的项目前缀（wallet service）
- **`<模块>`**: 功能模块分类
- **`名称`**: 具体表名

---

## 📋 现有表清单

### 用户认证模块（ws_auth）
| 表名 | 说明 | 备注 |
|------|------|------|
| `ws_users` | 用户信息表 | 核心表 |
| `ws_user_sessions` | 用户会话表 | JWT Token 管理 |

### 钱包管理模块（ws_wallet）
| 表名 | 说明 | 备注 |
|------|------|------|
| `wallets` | 钱包表 | 保持原名，逻辑上属于 ws 模块 |

**注意：** `wallets` 表是项目原有表，为保持向后兼容，不添加前缀。但通过 `user_id` 外键与 `ws_users` 关联。

---

## 🔗 表关系图

```
┌─────────────────┐
│   ws_users      │  ← 用户表（核心）
│                 │
│  - id           │
│  - username     │
│  - email        │
│  - password_hash│
│  - avatar_url   │
│  - created_at   │
│  - updated_at   │
│  - last_login   │
│  - is_active    │
└────────┬────────┘
         │
         │ 1:N (外键)
         │ ON DELETE CASCADE
         │
         ├──────────────────────┐
         │                      │
         ▼                      ▼
┌─────────────────┐    ┌─────────────────┐
│ ws_user_sessions│    │    wallets      │
│                 │    │                 │
│  - id           │    │  - id           │
│  - user_id (FK) │    │  - user_id (FK) │
│  - token        │    │  - address      │
│  - refresh_token│    │  - mnemonic_enc │
│  - expires_at   │    │  - private_key_enc│
│  - ip_address   │    │  - label        │
│  - user_agent   │    │  - tags         │
│  - created_at   │    │  - created_at   │
└─────────────────┘    └─────────────────┘
```

---

## 💡 设计理由

### 为什么使用 `ws_` 前缀？

#### ✅ 优点
1. **避免冲突** - 在共享测试库中不会与其他项目表名重复
2. **清晰归属** - 一眼就能看出是哪个项目的表
3. **便于管理** - 可以通过 `LIKE 'ws_%'` 快速查询所有相关表
4. **模块化** - 未来可以扩展更多模块前缀

#### ❌ 不使用前缀的问题
```sql
-- 如果多个项目都有 users 表
SHOW TABLES;
-- 看到：users, sessions, wallets...
-- 无法区分哪些是当前项目的！
```

---

## 🔍 查询示例

### 查看本项目所有表
```sql
-- 方法 1：按前缀筛选
SELECT 
    TABLE_NAME,
    TABLE_COMMENT
FROM INFORMATION_SCHEMA.TABLES
WHERE TABLE_SCHEMA = DATABASE()
AND TABLE_NAME LIKE 'ws_%'
ORDER BY TABLE_NAME;

-- 方法 2：直接列出
SHOW TABLES LIKE 'ws_%';
```

### 查看表结构
```sql
DESCRIBE ws_users;
DESCRIBE ws_user_sessions;
DESCRIBE wallets;
```

### 查看所有字段信息
```sql
SELECT 
    TABLE_NAME, 
    COLUMN_NAME, 
    DATA_TYPE, 
    IS_NULLABLE, 
    COLUMN_DEFAULT,
    COLUMN_COMMENT
FROM INFORMATION_SCHEMA.COLUMNS
WHERE TABLE_SCHEMA = DATABASE()
AND TABLE_NAME IN ('ws_users', 'ws_user_sessions', 'wallets')
ORDER BY TABLE_NAME, ORDINAL_POSITION;
```

---

## 📐 未来扩展

如果项目继续扩大，可以采用更细的模块划分：

### 价格提醒模块（ws_price_）
- `ws_price_alerts` - 价格提醒设置
- `ws_price_alert_history` - 提醒触发历史

### 投资组合模块（ws_portfolio_）
- `ws_portfolio_holdings` - 持仓记录
- `ws_portfolio_transactions` - 交易记录
- `ws_portfolio_snapshots` - 资产快照

### 通知模块（ws_notify_）
- `ws_notifications` - 站内通知
- `ws_notification_settings` - 通知设置

### 新闻模块（ws_news_）
- `ws_news` - 新闻文章
- `ws_news_read_history` - 阅读历史

---

## 🛠️ 迁移脚本

### 执行顺序
```bash
# 1. 运行主迁移脚本（创建 ws_开头的表）
mysql -u dev -p < database/migration_safe.sql

# 2. 验证结果
mysql -u dev -p -e "SHOW TABLES LIKE 'ws_%';"
```

### 回滚脚本（如果需要）
```sql
-- 删除所有 ws_ 开头的表
DROP TABLE IF EXISTS ws_user_sessions;
DROP TABLE IF EXISTS ws_users;

-- 注意：wallets 表不会被删除（保留原名）
```

---

## ⚠️ 注意事项

### 1. 外键约束
```sql
-- ✅ 正确：引用 ws_users
FOREIGN KEY (user_id) REFERENCES ws_users(id) ON DELETE CASCADE

-- ❌ 错误：引用不存在的表
FOREIGN KEY (user_id) REFERENCES users(id)  -- users 表不存在！
```

### 2. SQL 查询
```rust
// ✅ 正确：使用完整表名
sqlx::query("SELECT * FROM ws_users WHERE username = ?")

// ❌ 错误：忘记前缀
sqlx::query("SELECT * FROM users WHERE username = ?")  
// 会报错：Table 'xxx.users' doesn't exist
```

### 3. Rust 代码已更新
所有 SQL 查询已更新为使用 `ws_users`：
- ✅ `SELECT COUNT(*) FROM ws_users ...`
- ✅ `INSERT INTO ws_users ...`
- ✅ `SELECT * FROM ws_users WHERE ...`

---

## 📊 对比表

### 旧命名 vs 新命名

| 旧表名 | 新表名 | 说明 |
|--------|--------|------|
| `users` | `ws_users` | ✅ 已迁移 |
| `user_sessions` | `ws_user_sessions` | ✅ 已迁移 |
| `wallets` | `wallets` | ⚠️ 保持原名（向后兼容） |

---

## 🎯 最佳实践

### 1. 新项目表一律加前缀
```sql
CREATE TABLE ws_new_feature (
    id INT PRIMARY KEY,
    ...
);
```

### 2. 外键引用要准确
```sql
FOREIGN KEY (user_id) REFERENCES ws_users(id)
-- 不是 users(id)！
```

### 3. 代码中使用表别名
```rust
// 提高可读性
sqlx::query("SELECT u.* FROM ws_users u WHERE u.username = ?")
```

### 4. 注释中标注模块
```sql
-- ============================================
-- 用户认证模块 (ws_auth)
-- ============================================
```

---

## 📖 相关文档

- [AUTH_SYSTEM_GUIDE.md](AUTH_SYSTEM_GUIDE.md) - 用户认证系统使用指南
- [database/migration_safe.sql](database/migration_safe.sql) - 安全迁移脚本
- [database/auth_tables.sql](database/auth_tables.sql) - 建表脚本

---

## ✅ 总结

### 当前表结构
```
数据库：your_database_name
├── ws_users              ← 用户表（新增）
├── ws_user_sessions      ← 用户会话表（新增）
└── wallets               ← 钱包表（原有，已添加 user_id 字段）
```

### 命名规范
- ✅ 统一使用 `ws_` 前缀
- ✅ 避免与其他项目冲突
- ✅ 清晰的模块划分
- ✅ 便于管理和查询

---

**命名规范制定完成！** 🎉

这样可以确保在共享测试库中，你的表名不会与其他项目冲突！
