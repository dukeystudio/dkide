# Homebrew Cask 分发（都客AI助手 · dkide）

用户通过一条命令即可安装，无需开发者账号，Cask 会自动清除隔离属性以避开 Gatekeeper 警告。

## 用户安装方式

```bash
brew tap dukeystudio/tap
brew install --cask dkide
```

## 自动发布（推荐）

推送 `v*` tag 即触发 `.github/workflows/release.yml`，自动完成：arm64 构建 → 创建 GitHub Release → 生成 Cask → 推送 tap 仓库。当前仅发布 Apple Silicon (arm64) 版本。

```bash
git tag v1.1.0
git push origin v1.1.0
```

### 前置配置

1. **TAP_TOKEN**（可选）：在仓库 Settings → Secrets → Actions 新建 `TAP_TOKEN`，填入一个有 `dukeystudio/homebrew-tap` 仓库写权限的 Personal Access Token（fine-grained token 仅需该仓库 Contents 读写）。不配置则跳过自动推送 tap，工作流仍会构建 DMG 并创建 Release，Cask 需手动更新。
2. `GITHUB_TOKEN` 由 Actions 自动提供，用于创建 Release 和上传资产。
3. 工作流使用 `macos-latest`（arm64）单个 runner，仅构建 Apple Silicon 版本。若标签变更，到 GitHub 官方文档核对当前可用的 macOS runner 标签并替换。

工作流会从 tag 名（去掉 `v`）自动同步 `package.json`、`tauri.conf.json`、`Cargo.toml` 的版本号，无需手动改版本。

## 手动发布流程

### 1. 构建两个架构的 DMG

Tauri 默认构建当前架构。在 Apple Silicon 机器上构建 x86_64 需要 Rosetta 或交叉编译工具链，通常做法是分别在两台机器上构建：

```bash
npm run tauri:build
```

产物位于 `src-tauri/target/release/bundle/dmg/`，命名类似（中文产品名 + 含空格的旧版不同，现为纯中文）：

```
都客AI助手_1.0.0_aarch64.dmg
都客AI助手_1.0.0_x64.dmg
```

> 若 Tauri 生成的 x64 文件名不同（如 `x86_64`），以实际文件名为准。

### 2. 上传到 GitHub Releases

在 `https://github.com/dukeystudio/dkide/releases` 创建版本 `v1.0.0`，上传两个 DMG。

### 3. 生成 Cask

```bash
cd homebrew
chmod +x gen-cask.sh
./gen-cask.sh \
  "../src-tauri/target/release/bundle/dmg/都客AI助手_1.0.0_aarch64.dmg" \
  "../src-tauri/target/release/bundle/dmg/都客AI助手_1.0.0_x64.dmg" \
  1.0.0 \
  https://github.com/dukeystudio/dkide/releases/download/v1.0.0
```

脚本会自动计算 SHA256、把中文文件名做 URL percent 编码，并写入 `Casks/dkide.rb`。

### 4. 发布 tap 仓库

1. 在 GitHub 创建公开仓库 `homebrew-tap`（即 `dukeystudio/tap`）
2. 把本目录的 `Casks/` 文件夹推入该仓库
3. 用户即可通过 `brew tap dukeystudio/tap && brew install --cask dkide` 安装

## 更新版本

构建新 DMG → 上传 Release → 重跑 `gen-cask.sh` → 提交 Cask 到 tap 仓库即可。

## 说明

- `postflight` 执行 `xattr -c com.apple.quarantine`，移除隔离属性，使未签名应用可直接打开
- `zap` 定义卸载时清理的本地数据目录（identifier 仍为 `com.dkide.app`，保证老用户数据不丢失）
- 这种方式不依赖 Apple 开发者账号，适合早期分发；正式商业化仍建议注册 Developer ID 签名公证
