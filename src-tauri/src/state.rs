use std::sync::Mutex;

use crate::approval::ApprovalBroker;
use crate::cancel::CancelRegistry;
use crate::error::Result;
use crate::models::{AppConfig, Session};
use crate::secure_store::SecureStore;
use crate::skill::SkillManager;
use crate::store::Store;

pub struct AppState {
    pub store: Store,
    pub config: Mutex<AppConfig>,
    pub sessions: Mutex<Vec<Session>>,
    pub skill_manager: Mutex<SkillManager>,
    pub secure_store: SecureStore,
    pub approvals: ApprovalBroker,
    pub cancels: CancelRegistry,
    pub http: reqwest::Client,
}

impl AppState {
    pub fn new(store: Store) -> Result<Self> {
        store.init()?;
        crate::logger::init(&store.dir).ok();

        let mut config = store.load_config()?;
        let secure_store = SecureStore::new(&store.dir);
        // 把钥匙串中的真实 key 注入到内存配置（落盘的 config.json 中 api_key 始终为空）
        for provider in &mut config.providers {
            if let Some(key) = secure_store.get(&provider.id) {
                provider.api_key = key;
            }
            // 内置 Agnes 服务：key 随版本内置，用户无需填写
            if provider.api_key.is_empty() && crate::models::is_builtin_agnes(&provider.base_url) {
                provider.api_key = crate::models::BUILTIN_AGNES_API_KEY.into();
            }
        }

        let sessions = store.load_sessions()?;
        let mut skill_manager = SkillManager::new(store.skills_dir());

        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .map_err(|e| crate::error::AppError(e.to_string()))?;

        // 设置工作区
        if !config.workspace_path.trim().is_empty() {
            skill_manager.set_workspace(std::path::PathBuf::from(&config.workspace_path));
        }

        Ok(Self {
            store,
            config: Mutex::new(config),
            sessions: Mutex::new(sessions),
            skill_manager: Mutex::new(skill_manager),
            secure_store,
            approvals: ApprovalBroker::default(),
            cancels: CancelRegistry::default(),
            http,
        })
    }

    /// 保存配置：api_key 写入钥匙串，配置文件中只留空字符串。
    pub fn save_config(&self) -> Result<()> {
        let config = self.config.lock().unwrap();
        for provider in &config.providers {
            // 内置 Agnes 的 key 是随版本的内置凭据，不写入用户钥匙串，避免污染
            if provider.api_key == crate::models::BUILTIN_AGNES_API_KEY {
                continue;
            }
            self.secure_store.set(&provider.id, &provider.api_key)?;
        }
        let mut safe_config = config.clone();
        for provider in &mut safe_config.providers {
            provider.api_key.clear();
        }
        self.store.save_config(&safe_config)
    }

    pub fn save_sessions(&self) -> Result<()> {
        let sessions = self.sessions.lock().unwrap();
        self.store.save_sessions(&sessions)
    }
}
