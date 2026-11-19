# 都客AI助手（dkide）

本地优先的跨平台桌面 AI 助手，接入任意 **OpenAI 兼容** 模型中转站，具备**技能（Skills）扩展**与**内置工具调用**能力。

> 产品名称：**都客AI助手** · 英文标识：**dkide** · 应用标识符保持 `com.dkide.app`（兼容已安装用户数据）。

## 核心特性

### 对话
- SSE 流式输出，Markdown 渲染 + 代码语法高亮 + 一键复制
- 多会话管理：搜索、置顶、重命名、删除、导出 Markdown
- 消息操作：复制、编辑、删除、重新生成

### 模型接入
- 多中转站配置（base_url + api_key + model），一键模板（OpenAI / DeepSeek / Moonshot / 智谱）
- 连接测试、故障提示；API Key 存于**系统钥匙串**（带本地加密回退），配置文件不含明文密钥

### 工具与安全
- 内置 `read_file` / `write_file` / `list_files` / `run_command`
- 文件工具限制在工作目录内，阻止目录穿越
- 命令工具：黑名单拦截危险命令、30 秒超时、**用户审批弹窗**、可配置为自动放行或禁用
- 上下文 token 预算裁剪，避免长对话超限

### 技能系统
- `SKILL.md` 定义技能，模型按需通过 `activate_skill` 自动加载
- 安装方式：文件夹 / ZIP / ZIP URL / 技能市场一键安装
- 支持工作区级技能目录 `.dkide/skills/`

### 用量与体验
- 实时 token 与费用估算，全局用量统计
- 浅色 / 深色 / 跟随系统主题
- 首次启动配置向导，侧边栏可折叠
- 结构化文件日志（按日期滚动），配置版本迁移与损坏备份，原子写入

## 环境要求

- Node.js 18+，Rust stable，macOS / Windows / Linux 桌面环境

## 运行

```bash
npm install
npm run tauri:dev      # 开发模式
npm run tauri:build    # 生产打包（dmg / msi / AppImage）
```

验证：

```bash
npm run build                 # 前端类型检查 + 构建
cd src-tauri && cargo test    # Rust 单元测试（10 项）
```

## 快速开始

1. 首次启动进入配置向导，选择服务商预设（或手动填写 base_url / 模型名）
2. 填入 API Key → 测试连接 → 保存并进入
3. 在「模型设置 → 工具工作目录」指定允许 AI 操作的目录
4. 在「技能管理」安装示例技能 `examples/skills/hello-skill/` 体验自动加载

## 技能格式

```markdown
---
name: 我的技能
description: 技能描述（模型据此决定何时加载）
version: 1.0.0
author: me
allowed_tools: [read_file, write_file]
---

# 操作指令
模型加载该技能后应遵循的详细步骤和规则。
```

## 安全设计

| 措施 | 说明 |
|------|------|
| 密钥存储 | API Key 存系统钥匙串（macOS Keychain / Windows Credential Manager），配置文件仅存空值 |
| 路径隔离 | 文件工具限制在工作目录，阻止 `../` 越界 |
| 命令审批 | 写操作与命令执行需用户确认（可配置策略） |
| 命令黑名单 | 拦截 `rm -rf /`、`sudo`、`mkfs`、fork 炸弹等危险命令 |
| 命令超时 | 单条命令最长 30 秒 |
| Zip Slip 防护 | ZIP 安装校验路径，拒绝 `..` 条目 |
| 数据可靠 | 原子写入 + 配置版本迁移 + 损坏备份 |
| 隐私 | 对话与配置全部本地存储，不上传云端 |

## 项目结构

```
src-tauri/src/
  commands.rs      # 所有 Tauri commands
  llm/             # OpenAI 客户端、SSE 解析、工具循环
  tools/           # 内置工具（审批/黑名单/超时）
  skill/           # 技能解析与管理
  approval.rs      # 审批中心
  cancel.rs        # 生成取消
  usage.rs         # 用量与定价
  secure_store.rs  # 密钥存储
  store.rs         # 原子持久化 + 版本迁移
  logger.rs        # 文件日志
  config.rs        # 配置迁移
src/
  components/      # 对话/设置/技能/关于/审批/引导
  store.ts         # 状态管理
  markdown.ts      # Markdown + 代码高亮
  api.ts           # Tauri 封装
```

## 后续可扩展

- 技能市场在线索引（配置 `marketplace_url` 接入自有市场）
- 多模型并行对比、故障自动切换
- 云同步（需用户授权）
- 插件系统（自定义工具）
