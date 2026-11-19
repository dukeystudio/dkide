use std::collections::HashSet;
use std::sync::{Arc, Mutex};

/// 生成取消注册表：按会话id记录用户请求的取消。
#[derive(Clone, Default)]
pub struct CancelRegistry {
    ids: Arc<Mutex<HashSet<String>>>,
}

impl CancelRegistry {
    pub fn cancel(&self, session_id: &str) {
        self.ids.lock().unwrap().insert(session_id.to_string());
    }

    pub fn is_cancelled(&self, session_id: &str) -> bool {
        self.ids.lock().unwrap().contains(session_id)
    }

    pub fn clear(&self, session_id: &str) {
        self.ids.lock().unwrap().remove(session_id);
    }
}
