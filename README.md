# 🚀 钱包服务 - 完整功能版

一个功能丰富的加密货币钱包管理系统，配备现代化的 Web 界面和实时价格监控。

**⚠️ 重要提示**：这是一个测试/娱乐项目，所有余额和交易数据都是模拟的，仅供学习和娱乐！

---

## ✨ 功能特性

### 🔐 核心功能
- **生成安全的钱包**：BIP39 助记词 + secp256k1 加密
- **AES-GCM 加密存储**：保护敏感数据
- **以太坊地址生成**：Keccak256 哈希算法
- **MySQL 数据库持久化**：安全可靠的数据存储

### 💰 实时价格监控（新！）
- **WebSocket 实时推送** - 连接 CryptoCompare，毫秒级更新
- **多币种价格** - ETH、BTC、TRX、BNB 实时展示
- **首页集成** - 打开页面即可看到价格跳动
- **自动重连** - 网络断开后自动恢复

### 🎮 娱乐功能
- 🎰 **幸运抽奖** - 试试手气，看能否抽到幸运数字
- 🏆 **成就系统** - 收集 5 个成就徽章
- 💰 **余额模拟** - 假装查询 ETH 和 USD 余额
- 📜 **交易记录** - 自动生成假的交易历史
- 🗑️ **一键清空** - 删除所有数据重新开始
- 🏷️ **标签管理** - 给钱包分类标记

---

## 🛠️ 技术栈

- **后端框架**：Axum (Rust)
- **前端**：HTML5 + CSS3 + JavaScript
- **数据库**：MySQL
- **加密**：AES-GCM, secp256k1, SHA3
- **运行时**：Tokio
- **WebSocket**：tokio-tungstenite (native-tls)
- **数据源**：CryptoCompare WebSocket API

---

## 📦 快速开始

### 1. 环境准备
```bash
# 需要安装
- Rust (最新稳定版)
- MySQL 数据库
- Git
```

### 2. 配置环境变量
编辑 `.env` 文件：
```env
DATABASE_URL=mysql://用户名：密码@主机：端口/数据库名
ENCRYPTION_KEY=12345678901234567890123456789012
ENVIRONMENT=dev
```

### 3. 创建数据库表
```sql
CREATE DATABASE IF NOT EXISTS your_database;
USE your_database;

CREATE TABLE wallets (
    id INT AUTO_INCREMENT PRIMARY KEY,
    address VARCHAR(255) NOT NULL UNIQUE,
    mnemonic_enc TEXT NOT NULL,
    private_key_enc TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
```

### 4. 启动服务
```bash
dx serve
# 或
cargo run
```

### 5. 访问页面
打开浏览器：**http://127.0.0.1:3000**

---

## 🎯 API 端点总览

### 基础功能
| 方法 | 路径 | 描述 |
|------|------|------|
| GET | `/` | Web 主界面（含实时价格） |
| GET | `/ws` | WebSocket 端点（实时价格推送） |
| GET | `/health` | 健康检查 |
| GET | `/stats` | 统计信息 |
| POST | `/wallets` | 创建钱包 |
| GET | `/wallets` | 钱包列表（前 10 个） |
| GET | `/wallets/{address}` | 钱包详情 |
| DELETE | `/wallets/{address}` | 删除钱包 |
| POST | `/wallets/batch` | 批量创建 |
| GET | `/wallets/search?q=` | 搜索钱包 |
| GET | `/wallets/export` | 导出钱包 |
| POST | `/wallets/import` | 导入助记词 |
| POST | `/wallets/batch-delete` | 批量删除 |

### 💰 市场数据
| 方法 | 路径 | 描述 |
|------|------|------|
| GET | `/market/prices` | 获取加密货币价格 |
| GET | `/market/trends` | 获取市场趋势 |
| POST | `/alerts/price` | 设置价格提醒 |
| GET | `/portfolio/{address}` | 计算资产组合 | |

### 🆕 娱乐功能
| 方法 | 路径 | 描述 |
|------|------|------|
| POST | `/wallets/{address}/tags` | 添加标签 |
| GET | `/wallets/{address}/tags` | 获取标签 |
| GET | `/wallets/{address}/balance` | 模拟余额 |
| GET | `/wallets/{address}/transactions` | 交易记录 |
| GET | `/lucky-draw` | 幸运抽奖 |
| GET | `/achievements` | 成就系统 |
| DELETE | `/admin/clear-all` | 一键清空 |

---

## 🎮 玩法指南

### 新手入门
1. 打开页面查看实时价格（顶部区域）
2. 点击"➕ 单个创建"或"🎯 批量创建"
3. 查看钱包列表，点击"💰"看余额
4. 点击"📜"查看交易记录
5. 点击"🏷️"添加标签分类

### 实时价格监控
- **自动连接** - 打开页面自动连接 WebSocket
- **实时跳动** - 价格毫秒级更新，无需刷新
- **状态指示** - 🟢 已连接 / 🔴 断开 / 🟠 连接中
- **多币种** - ETH、BTC、TRX、BNB 同时展示

### 娱乐模式
1. **试试手气** - 点击"🎰 幸运抽奖"
2. **收集成就** - 点击"🏆 成就系统"
3. **清空重来** - 点击"🗑️ 一键清空"（需二次确认）

### 成就列表
- 🎖️ **新手上路** - 创建第 1 个钱包
- 🚀 **批量生产** - 累计创建 10 个钱包
- 💎 **收藏家** - 拥有 50 个钱包
- 👑 **百夫长** - 拥有 100 个钱包
- ⭐ **今日之星** - 今天创建 5 个以上

---

## ⚠️ 免责声明

### 重要提示
- 💡 **余额是假的** - 仅用于娱乐的模拟数据
- 💡 **交易是假的** - 自动生成的虚假记录
- 💡 **仅供测试** - 不要用于真实场景
- ❌ **非生产级** - 这是学习/测试项目

### 安全机制
- ⚠️ 清空需要二次确认
- ⚠️ 删除不可恢复
- ⚠️ 仅限开发环境使用

---

## 📁 项目结构

```
wallet-service/
├── src/
│   └── main.rs          # 主程序（包含所有功能）
├── index.html           # Web 界面（含所有交互）
├── .env                 # 环境变量
├── Cargo.toml           # 依赖配置
└── README.md            # 本文档
```

---

## 🔒 安全说明

### 开发模式 (`ENVIRONMENT=dev`)
- ✅ 返回明文助记词和私钥
- ⚠️ 仅用于测试
- ❌ 不要用于生产

### 生产模式 (`ENVIRONMENT=prod`)
- ❌ 不返回明文敏感信息
- ✅ 所有数据加密存储
- 🔐 符合安全最佳实践

---

## 💡 常见问题

### Q: 价格是真的吗？
A: 是的！通过 WebSocket 连接 CryptoCompare API，获取真实实时价格。

### Q: 为什么余额显示那么多？
A: 那是模拟数据，根据地址哈希生成的固定值，仅供娱乐。

### Q: 交易记录是真的吗？
A: 不是，是自动生成的假数据，每笔交易都是随机的。

### Q: 可以删除所有数据吗？
A: 可以，点击"🗑️ 一键清空"，但需要二次确认，删除后无法恢复。

### Q: 幸运抽奖怎么玩？
A: 每次抽奖会生成一个新钱包，如果地址包含 6 或 8 就中奖。

### Q: 成就是什么？
A: 收集类徽章，通过创建钱包的数量和行为解锁。

---

## 🎊 特色亮点

1. **完整的功能体系** - 28 个 API 端点
2. **现代化 UI** - 渐变色彩、响应式设计
3. **实时价格监控** - WebSocket 推送、毫秒级更新
4. **游戏化体验** - 抽奖、成就、排行榜
5. **安全可靠** - AES-GCM 加密、二次确认
6. **中文友好** - 全部中文提示
7. **即开即用** - 无需复杂配置

---

## 🚀 立即体验

```bash
# 启动服务
dx serve

# 访问页面
http://127.0.0.1:3000
```

**祝你玩得开心！** 🎉

---

*最后更新：2024-01-01*  
*状态：✅ 所有功能已完成（含 WebSocket 实时价格）*
