cask "dkide" do
  version "1.0.0"

  # 都客AI助手 (dkide) — Tauri 产物命名: "都客AI助手_<version>_<arch>.dmg"
  # "都客AI助手" 的 URL 编码为 %E9%83%BD%E5%AE%A2AI%E5%8A%A9%E6%89%8B
  on_arm do
    url "https://github.com/dukeystudio/dkide/releases/download/v#{version}/%E9%83%BD%E5%AE%A2AI%E5%8A%A9%E6%89%8B_#{version}_aarch64.dmg"
    sha256 "6a04d48e96e829c652b79911cbacfef2eb4bcbc37ee56c7875da223e0f5531f5"
  end
  on_intel do
    url "https://github.com/dukeystudio/dkide/releases/download/v#{version}/%E9%83%BD%E5%AE%A2AI%E5%8A%A9%E6%89%8B_#{version}_x64.dmg"
    sha256 "REPLACE_WITH_X64_SHA256"
  end

  name "都客AI助手"
  name "dkide"
  desc "Local-first desktop AI assistant with skills and tools"
  homepage "https://github.com/dukeystudio/dkide"

  app "都客AI助手.app"

  # 应用未签名/未公证：移除下载隔离属性，避免"无法打开"警告
  postflight do
    system_command "/usr/bin/xattr",
                   args: ["-c", "com.apple.quarantine", "#{appdir}/都客AI助手.app"],
                   sudo: false
  end

  # 卸载时清理应用数据（identifier 仍为 com.dkide.app）
  zap trash: [
    "~/Library/Application Support/com.dkide.app",
    "~/Library/Logs/dkide",
    "~/Library/Saved Application State/com.dkide.app.savedState",
  ]
end
