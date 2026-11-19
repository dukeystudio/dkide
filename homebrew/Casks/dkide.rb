cask "dkide" do
  version "1.1.0"

  # 仅提供 arm64 (Apple Silicon) 版本
  url "https://github.com/dukeystudio/dkide/releases/download/v#{version}/dkide_#{version}_aarch64.dmg"
  sha256 "REPLACE_WITH_AARCH64_SHA256"

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
