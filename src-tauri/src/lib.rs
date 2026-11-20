mod approval;
mod auth;
mod cancel;
mod commands;
mod config;
mod error;
mod llm;
mod logger;
mod models;
mod secure_store;
mod skill;
mod state;
mod store;
mod tools;
mod usage;

use tauri::Manager;

use crate::error::AppError;
use crate::state::AppState;
use crate::store::Store;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_dir = app.path().app_data_dir().expect("无法获取应用数据目录");
            let state = AppState::new(Store::new(app_dir)).map_err(|e| AppError(e.to_string()))?;
            let http = state.http.clone();
            app.manage(state);
            app.manage(http);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            auth::auth_login,
            auth::auth_register,
            auth::auth_verify,
            auth::auth_logout,
            auth::auth_session,
            auth::auth_quota,
            commands::get_config,
            commands::save_config,
            commands::test_provider,
            commands::list_sessions,
            commands::create_session,
            commands::delete_session,
            commands::rename_session,
            commands::toggle_pin_session,
            commands::append_message,
            commands::delete_message,
            commands::update_message,
            commands::export_session,
            commands::chat,
            commands::cancel_chat,
            commands::read_file_for_preview,
            commands::read_text_attachment,
            commands::open_file_in_os,
            commands::list_approvals,
            commands::respond_approval,
            commands::get_total_usage,
            commands::list_skills,
            commands::install_skill_from_dir,
            commands::install_skill_from_zip,
            commands::install_skill_from_url,
            commands::uninstall_skill,
            commands::open_skills_dir,
            commands::list_marketplace,
            commands::get_app_info,
            commands::get_project_root,
            commands::create_project,
        ])
        .run(tauri::generate_context!())
        .expect("Tauri 应用运行失败");
}
