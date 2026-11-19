#!/usr/bin/env bash
# 根据给定的两个 DMG 文件生成 Cask（自动计算 SHA256、URL 编码中文名）
# 用法: ./gen-cask.sh <aarch64.dmg> <x64.dmg> <version> <release_base_url>
# 例:   ./gen-cask.sh "都客AI助手_1.0.0_aarch64.dmg" "都客AI助手_1.0.0_x64.dmg" 1.0.0 \
#         https://github.com/dukeystudio/dkide/releases/download/v1.0.0
set -euo pipefail

AARCH64_DMG="${1:?请提供 aarch64 DMG 路径}"
X64_DMG="${2:?请提供 x64 DMG 路径}"
VERSION="${3:?请提供版本号}"
BASE_URL="${4:?请提供 release 文件所在的基础 URL}"

[ -f "$AARCH64_DMG" ] || { echo "找不到: $AARCH64_DMG"; exit 1; }
[ -f "$X64_DMG" ] || { echo "找不到: $X64_DMG"; exit 1; }

SHA_A=$(shasum -a 256 "$AARCH64_DMG" | awk '{print $1}')
SHA_X=$(shasum -a 256 "$X64_DMG" | awk '{print $1}')
# 对文件名做 URL percent 编码（空格->%20，中文->%XX）
enc() { node -e 'process.stdout.write(encodeURIComponent(process.argv[1]))' "$1"; }
A_NAME=$(enc "$(basename "$AARCH64_DMG")")
X_NAME=$(enc "$(basename "$X64_DMG")")

cat > Casks/dkide.rb <<EOF
cask "dkide" do
  version "$VERSION"

  on_arm do
    url "$BASE_URL/$A_NAME"
    sha256 "$SHA_A"
  end
  on_intel do
    url "$BASE_URL/$X_NAME"
    sha256 "$SHA_X"
  end

  name "都客AI助手"
  name "dkide"
  desc "Local-first desktop AI assistant with skills and tools"
  homepage "https://github.com/dukeystudio/dkide"

  app "都客AI助手.app"

  postflight do
    system_command "/usr/bin/xattr",
                   args: ["-c", "com.apple.quarantine", "\#{appdir}/都客AI助手.app"],
                   sudo: false
  end

  zap trash: [
    "~/Library/Application Support/com.dkide.app",
    "~/Library/Logs/dkide",
    "~/Library/Saved Application State/com.dkide.app.savedState",
  ]
end
EOF

echo "已生成 Casks/dkide.rb"
echo "aarch64 sha256: $SHA_A"
echo "x64 sha256:     $SHA_X"
