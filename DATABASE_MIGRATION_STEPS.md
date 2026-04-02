# 📦 数据库迁移执行指南

## ✅ 问题已修复

**修复内容**：移除了 FOREIGN KEY 约束后的 COMMENT，因为 MySQL 不支持此外键语法。

**修复位置**：
- `ws_user_sessions` 表的外键（第 41 行）
- `ws_wallets` 表的外键（第 62 行）

---

## 🚀 执行迁移（三种方法）

### 方法一：使用 PowerShell 脚本（推荐）

```powershell
cd d:\projet\cargo\wallet-service\database
.\run_migration.ps1
```

---

### 方法二：使用 MySQL 命令行工具

如果你安装了 MySQL 客户端：

```bash
mysql -h 192.168.0.23 -u dev -pdev9856AC zbs < database/migration.sql
```

或者在 Git Bash 中：

```bash
cd /d/projet/cargo/wallet-service
mysql -h 192.168.0.23 -u dev -pdev9856AC zbs < database/migration.sql
```

---

### 方法三：使用图形化工具

#### MySQL Workbench:
1. 打开 MySQL Workbench
2. 连接到数据库 `192.168.0.23` (用户：dev, 密码：dev9856AC)
3. 选择数据库 `zbs`
4. 打开文件 `File > Open SQL Script...`
5. 选择 `d:\projet\cargo\wallet-service\database\migration.sql`
6. 点击执行按钮（⚡）

#### Navicat:
1. 连接到数据库
2. 右键点击 `zbs` 数据库
3. 选择 `运行 SQL 文件...`
4. 选择 `migration.sql` 文件
5. 点击开始

---

## 📋 验证迁移结果

迁移成功后，执行以下 SQL 验证：

```sql
-- 查看所有 ws_ 开头的表
SHOW TABLES LIKE 'ws_%';

-- 预期结果：
-- ws_users
-- ws_user_sessions
-- ws_wallets

-- 查看用户表结构
DESCRIBE ws_users;

-- 查看会话表结构
DESCRIBE ws_user_sessions;

-- 查看钱包表结构
DESCRIBE ws_wallets;

-- 查看测试用户（密码：123456）
SELECT * FROM ws_users WHERE username = 'testuser';
```

---

## 🔍 预期输出

成功的迁移应该显示：

```
=====================================
Migration completed successfully!
=====================================

+------------------+----------------------+
| TABLE_NAME       | TABLE_COMMENT        |
+------------------+----------------------+
| ws_user_sessions | 用户会话表           |
| ws_users         | 用户表               |
| ws_wallets       | 钱包表               |
+------------------+----------------------+

+ total_users |
+-------------+
|           1 |
+-------------+

+ total_wallets |
+---------------+
|             0 |
+---------------+
```

---

## ⚠️ 常见问题

### 1. 连接失败

**错误**：`Can't connect to MySQL server`

**解决**：
- 检查数据库服务器是否运行
- 确认 IP 地址和端口正确（192.168.0.23:3306）
- 检查防火墙设置

### 2. 权限不足

**错误**：`Access denied for user 'dev'@'...'`

**解决**：
- 确认用户名和密码正确
- 联系 DBA 授予 CREATE TABLE 权限

### 3. 表已存在

**错误**：`Table 'ws_users' already exists`

**解决**：
- 这是正常的，说明表已经创建过
- SQL 使用了 `CREATE TABLE IF NOT EXISTS`，不会重复创建
- 如需重新创建，先删除旧表：
  ```sql
  DROP TABLE IF EXISTS ws_wallets;
  DROP TABLE IF EXISTS ws_user_sessions;
  DROP TABLE IF EXISTS ws_users;
  ```

### 4. 外键约束失败

**错误**：`Cannot add foreign key constraint`

**解决**：
- 确保父表（ws_users）先创建
- 检查两个表的字符集是否一致（都应该是 utf8mb4）
- 确保外键字段类型相同（都是 INT）

---

## 🎯 下一步

迁移成功后，编译并启动服务：

```bash
# 编译项目
cargo build

# 或者使用 dx 工具
dx serve
```

然后测试用户注册 API：

```bash
curl -X POST http://localhost:8080/api/register \
  -H "Content-Type: application/json" \
  -d '{
    "username": "newuser",
    "email": "newuser@example.com",
    "password": "password123"
  }'
```

---

## 📞 需要帮助？

如果遇到问题，请提供：
1. 完整的错误信息
2. 使用的 MySQL 版本
3. 执行的命令

祝迁移顺利！🎉
