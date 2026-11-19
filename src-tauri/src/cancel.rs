use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use tokio::sync::Notify;

/// 生成取消注册表：按会话 id 记录用户请求的取消，并支持异步等待取消信号。
#[derive(Clone, Default)]
pub struct CancelRegistry {
    ids: Arc<Mutex<HashSet<String>>>,
    notifiers: Arc<Mutex<HashMap<String, Arc<Notify>>>>,
}

impl CancelRegistry {
    pub fn cancel(&self, session_id: &str) {
        self.ids.lock().unwrap().insert(session_id.to_string());
        if let Some(notifier) = self.notifiers.lock().unwrap().get(session_id) {
            notifier.notify_waiters();
        }
    }

    pub fn is_cancelled(&self, session_id: &str) -> bool {
        self.ids.lock().unwrap().contains(session_id)
    }

    pub fn clear(&self, session_id: &str) {
        self.ids.lock().unwrap().remove(session_id);
        self.notifiers.lock().unwrap().remove(session_id);
    }

    /// 等待当前会话收到取消信号；用于中断正在等待网络响应的生成任务。
    pub async fn wait_cancelled(&self, session_id: &str) {
        let notifier = {
            let mut notifiers = self.notifiers.lock().unwrap();
            notifiers
                .entry(session_id.to_string())
                .or_insert_with(|| Arc::new(Notify::new()))
                .clone()
        };
        if self.is_cancelled(session_id) {
            return;
        }
        loop {
            notifier.notified().await;
            if self.is_cancelled(session_id) {
                return;
            }
        }
    }
}
