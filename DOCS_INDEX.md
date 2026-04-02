# 📖 文档索引

这是钱包服务项目的文档导航页面。

---

## 🎯 核心文档（3 个）

### 1. [README.md](README.md) - 项目概览
**适合人群：** 所有人  
**阅读时间：** 5 分钟

**内容包含：**
- ✨ 功能特性介绍
- 🛠️ 技术栈说明
- 📦 快速开始指南
- 🎯 API 端点精选
- 🎮 玩法指南
- ❓ 常见问题

**什么时候看？**
- 第一次接触项目时
- 想了解项目能做什么
- 需要查阅 API 列表

---

### 2. [QUICK_START.md](QUICK_START.md) - 快速开始
**适合人群：** 新手用户  
**阅读时间：** 10 分钟

**内容包含：**
- 📋 环境准备清单
- 🔧 详细配置步骤
- 🚀 启动教程
- 💻 第一个钱包创建
- 🎨 Web 界面使用

**什么时候看？**
- 准备运行项目时
- 需要逐步操作指导
- 遇到配置问题时

---

### 3. [CODE_EXPLANATION.md](CODE_EXPLANATION.md) - 代码学习指南
**适合人群：** 开发者、学习者  
**阅读时间：** 30-60 分钟

**内容包含：**
- 📚 项目架构解析
- 🔍 核心代码详解（带注释）
- 💡 关键概念解释
- 📊 完整请求流程
- 🔐 安全性说明
- 🎓 学习建议

**什么时候看？**
- 想学习 Rust 区块链开发
- 需要理解代码实现原理
- 想要扩展或修改代码

---

## 🗂️ 其他文件

### 启动脚本
- **start.bat** - Windows 一键启动脚本

### 配置文件
- **.env** - 环境变量配置
- **Cargo.toml** - Rust 依赖配置

### 源代码
- **src/main.rs** - 后端主程序（所有功能）
- **index.html** - Web 前端界面
- **price_monitor.html** - 独立监控页面

---

## 🎓 推荐学习路径

### 路径一：快速体验（15 分钟）
```
1. 阅读 README.md (5 分钟)
2. 运行 start.bat (2 分钟)
3. 打开浏览器体验功能 (8 分钟)
```

### 路径二：系统学习（1-2 小时）
```
1. README.md → 了解项目 (10 分钟)
2. QUICK_START.md → 运行项目 (15 分钟)
3. CODE_EXPLANATION.md → 学习代码 (45-60 分钟)
4. 阅读 src/main.rs → 深入理解 (30 分钟+)
```

### 路径三：开发实战（按需查阅）
```
1. README.md → 查阅 API 端点
2. CODE_EXPLANATION.md → 查看相关代码段
3. src/main.rs → 定位具体实现
4. 修改并测试
```

---

## 📌 快速查找表

| 我想知道... | 查看文档 | 章节 |
|------------|---------|------|
| 项目能做什么 | README.md | 核心功能 |
| 如何运行项目 | QUICK_START.md | 启动教程 |
| 如何创建钱包 | QUICK_START.md | 创建第一个钱包 |
| WebSocket 原理 | CODE_EXPLANATION.md | WebSocket 实时价格推送 |
| 加密解密实现 | CODE_EXPLANATION.md | AES-GCM 加密解密 |
| 如何生成地址 | CODE_EXPLANATION.md | 生成钱包的核心逻辑 |
| API 有哪些 | README.md | API 端点精选 |
| 代码如何组织 | CODE_EXPLANATION.md | 项目架构解析 |

---

## 💡 使用技巧

### 1. 善用 Ctrl+F
在文档中快速搜索关键词：
- "WebSocket" - 实时价格推送
- "加密" - AES-GCM 实现
- "API" - 接口定义

### 2. 对照源码阅读
打开两个窗口：
- 左边：`src/main.rs`
- 右边：`CODE_EXPLANATION.md`

### 3. 实践优先
1. 先运行起来看看效果
2. 再深入学习原理
3. 最后尝试修改代码

---

## 🔗 外部资源

- [Rust 官方文档](https://doc.rust-lang.org/book/)
- [Axum 框架文档](https://github.com/tokio-rs/axum)
- [BIP39 标准](https://github.com/bitcoin/bips/blob/master/bip-0039.mediawiki)
- [以太坊地址生成](https://ethereum.org/en/developers/docs/accounts/)

---

**祝你学习愉快！** 🚀

*最后更新：2024-01-01*
