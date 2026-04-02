# 🗄️ 数据库迁移指南

## 📋 步骤一：执行迁移脚本

### 方法一：命令行执行（推荐）

```bash
# Windows (Git Bash)
mysql -u dev -p < database/migration.sql
# 输入密码：dev9856AC

# 或直接指定参数
mysql -h 192.168.0.23 -u dev -pdev9856AC < database/migration.sql
```

### 方法二：MySQL Workbench

1. 打开 MySQL Workbench
2. 连接到数据库服务器
3. 打开 `database/migration.sql` 文件
4. 执行所有语句（Lightning 图标）

### 方法三：手动复制粘贴

```sql
-- 在 MySQL 客户端中逐段执行
USE zbs;

-- 创建用户表
CREATE TABLE IF NOT EXISTS ws_users (...);

-- 创建会话表
CREATE TABLE IF NOT EXISTS ws_user_sessions (...);

-- 创建钱包表
CREATE TABLE IF NOT EXISTS ws_wallets (...);
```

---

## ✅ 验证迁移结果

### 查看所有表
```sql
SHOW TABLES LIKE 'ws_%';
```

**预期输出：**
```
+--------------------+
| Tables_in_zbs      |
+--------------------+
| ws_user_sessions   |
| ws_users           |
| ws_wallets         |
+--------------------+
```

### 查看表结构
```sql
DESCRIBE ws_users;
DESCRIBE ws_user_sessions;
DESCRIBE ws_wallets;
```

### 查看测试数据
```sql
SELECT * FROM ws_users WHERE username = 'testuser';
```

---

## 🔧 常见问题

### Q1: 表已存在错误
```sql
ERROR 1050 (42S01): Table 'ws_users' already exists
```

**解决方案：** 使用 `IF NOT EXISTS` 的迁移脚本已经处理了这种情况，可以直接运行。

---

### Q2: 外键约束失败
```sql
ERROR 1005 (HY000): Can't create table `ws_user_sessions` (errno: 150)
```

**原因：** 外键引用的表不存在或引擎不匹配。

**解决方案：**
1. 确保先创建 `ws_users` 表
2. 所有表使用相同的存储引擎（InnoDB）

---

### Q3: 字符集问题
```sql
ERROR 1366 (HY000): Incorrect string value
```

**解决方案：** 确保数据库和表都使用 utf8mb4 字符集：
```sql
ALTER DATABASE zbs CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;
```

---

## 🔄 回滚操作（如果需要）

### 删除所有 ws_ 表
```sql
DROP TABLE IF EXISTS ws_user_sessions;
DROP TABLE IF EXISTS ws_wallets;
DROP TABLE IF EXISTS ws_users;
```

**⚠️ 警告：** 这将删除所有相关数据且不可恢复！

---

## 📊 表关系说明

```
ws_users (用户表)
    │
    │ 1:N
    ├──────────────────────┐
    │                      │
    ▼                      ▼
ws_user_sessions      ws_wallets
(会话管理)           (钱包管理)
```

### 外键约束
- `ws_user_sessions.user_id` → `ws_users.id` (ON DELETE CASCADE)
- `ws_wallets.user_id` → `ws_users.id` (ON DELETE SET NULL)

---

## 🎯 下一步

迁移完成后，启动服务测试：

```bash
# 编译项目
cargo build

# 启动服务
dx serve
# 或
cargo run

# 访问 Web 界面
http://127.0.0.1:3000
```

---

## 📝 快速测试 API

### 注册用户
```bash
curl -X POST http://127.0.0.1:3000/auth/register \
  -H "Content-Type: application/json" \
  -d '{"username":"alice","email":"alice@test.com","password":"123456"}'
```

### 登录
```bash
curl -X POST http://127.0.0.1:3000/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"alice","password":"123456"}'
```

### 创建钱包
```bash
curl -X POST http://127.0.0.1:3000/wallets \
  -H "Content-Type: application/json"
```

---

## ✅ 完成检查清单

- [ ] 成功执行 migration.sql
- [ ] 看到 3 个 ws_ 开头的表
- [ ] ws_users 表中有测试用户
- [ ] 所有外键约束正确创建
- [ ] 服务可以正常启动
- [ ] 可以注册/登录用户
- [ ] 可以创建钱包

---

**祝你迁移顺利！** 🚀

如有问题，请查看主文档 README.md。
