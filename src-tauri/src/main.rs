// Windows 下 release 隐藏控制台窗口，避免双击运行时弹出黑色命令行窗口。
// 用 Rust 属性实现，无需 PowerShell / cmd 脚本，也不需要额外权限。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg_attr(mobile, tauri::mobile_entry_point)]
fn main() {
    dkide::run();
}
