-- ============================================
-- 钱包服务 - 数据库迁移脚本
-- 项目：wallet-service (ws_)
-- 命名规范：ws_<模块>_名称
-- 
-- 使用方法：
-- mysql -u 用户名 -p < database/migration.sql
-- ============================================

USE zbs;  -- TODO: 替换为你的数据库名

-- ============================================
-- 第一部分：用户认证模块 (ws_auth)
-- ============================================

-- 1. 用户表
CREATE TABLE IF NOT EXISTS ws_users (
    id INT AUTO_INCREMENT PRIMARY KEY COMMENT '用户 ID',
    username VARCHAR(50) UNIQUE NOT NULL COMMENT '用户名',
    email VARCHAR(100) UNIQUE NOT NULL COMMENT '邮箱',
    password_hash VARCHAR(255) NOT NULL COMMENT '密码哈希',
    avatar_url VARCHAR(255) COMMENT '头像 URL',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT '更新时间',
    last_login TIMESTAMP NULL COMMENT '最后登录时间',
    is_active BOOLEAN DEFAULT TRUE COMMENT '是否激活',
    INDEX idx_username (username),
    INDEX idx_email (email)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COMMENT='用户表';

-- 2. 用户会话表（用于 Token 管理）
CREATE TABLE IF NOT EXISTS ws_user_sessions (
    id INT AUTO_INCREMENT PRIMARY KEY COMMENT '会话 ID',
    user_id INT NOT NULL COMMENT '用户 ID',
    token VARCHAR(255) UNIQUE NOT NULL COMMENT 'JWT 令牌',
    refresh_token VARCHAR(255) COMMENT '刷新令牌',
    expires_at TIMESTAMP NOT NULL COMMENT '过期时间',
    ip_address VARCHAR(45) COMMENT 'IP 地址',
    user_agent TEXT COMMENT '用户代理',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
    FOREIGN KEY (user_id) REFERENCES ws_users(id) ON DELETE CASCADE,
    INDEX idx_token (token),
    INDEX idx_expires (expires_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COMMENT='用户会话表';

-- ============================================
-- 第二部分：钱包管理模块 (ws_wallet)
-- ============================================

-- 3. 创建钱包表（如果不存在）
CREATE TABLE IF NOT EXISTS ws_wallets (
    id INT AUTO_INCREMENT PRIMARY KEY COMMENT '钱包 ID',
    user_id INT COMMENT '所属用户 ID',
    address VARCHAR(255) NOT NULL UNIQUE COMMENT '钱包地址',
    mnemonic_enc TEXT NOT NULL COMMENT '加密的助记词',
    private_key_enc TEXT NOT NULL COMMENT '加密的私钥',
    label VARCHAR(100) COMMENT '钱包标签/备注',
    tags JSON COMMENT '标签数组（JSON 格式）',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
    INDEX idx_user_id (user_id),
    INDEX idx_address (address),
    FOREIGN KEY (user_id) REFERENCES ws_users(id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COMMENT='钱包表';

-- ============================================
-- 第三部分：测试数据（可选）
-- ============================================

-- 插入一个测试用户（密码：123456）
-- 注意：实际应该通过 API 注册，这里仅用于测试
INSERT INTO ws_users (username, email, password_hash) 
VALUES (
    'testuser',
    'test@example.com',
    '$2b$12$LQv3c1yqBWVHxkd0LHAkCOYz6TtxMQJqhN8/LewY5GyYzS3MebAJu'  -- bcrypt('123456')
)
ON DUPLICATE KEY UPDATE username=username;

-- ============================================
-- 第四部分：验证迁移结果
-- ============================================

SELECT '=====================================' AS '';
SELECT 'Migration completed successfully!' AS 'Status';
SELECT '=====================================' AS '';

-- 显示所有 ws_ 开头的表（本项目所有表）
SELECT 
    TABLE_NAME,
    TABLE_COMMENT
FROM INFORMATION_SCHEMA.TABLES
WHERE TABLE_SCHEMA = DATABASE()
AND TABLE_NAME LIKE 'ws_%'
ORDER BY TABLE_NAME;

-- 查看所有相关表结构
SELECT 
    TABLE_NAME, 
    COLUMN_NAME, 
    DATA_TYPE, 
    IS_NULLABLE, 
    COLUMN_DEFAULT,
    COLUMN_COMMENT
FROM INFORMATION_SCHEMA.COLUMNS
WHERE TABLE_SCHEMA = DATABASE()
AND TABLE_NAME IN ('ws_users', 'ws_user_sessions', 'ws_wallets')
ORDER BY TABLE_NAME, ORDINAL_POSITION;

-- ============================================
-- 第五部分：快速查询示例
-- ============================================

-- 查看用户总数
SELECT COUNT(*) AS total_users FROM ws_users;

-- 查看钱包总数
SELECT COUNT(*) AS total_wallets FROM ws_wallets;

-- 查看每个用户的钱包数量
SELECT 
    u.username,
    COUNT(w.id) AS wallet_count
FROM ws_users u
LEFT JOIN ws_wallets w ON u.id = w.user_id
GROUP BY u.id, u.username
ORDER BY wallet_count DESC;
