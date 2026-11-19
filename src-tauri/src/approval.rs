use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;

/// 待审批项
#[derive(Clone, Serialize)]
pub struct PendingApproval {
    pub id: String,
    pub tool: String,
    pub summary: String,
    #[serde(skip)]
    pub created: Instant,
}

/// 命令执行审批中心：工具调用前登记，前端弹窗后回调放行/拒绝。
#[derive(Clone, Default)]
pub struct ApprovalBroker {
    pending: Arc<Mutex<HashMap<String, PendingApproval>>>,
    decisions: Arc<Mutex<HashMap<String, bool>>>,
}

impl ApprovalBroker {
    pub fn request(&self, tool: &str, summary: impl Into<String>) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        self.pending.lock().unwrap().insert(
            id.clone(),
            PendingApproval {
                id: id.clone(),
                tool: tool.to_string(),
                summary: summary.into(),
                created: Instant::now(),
            },
        );
        self.decisions.lock().unwrap().remove(&id);
        id
    }

    /// 清理超过 5 分钟未处理的审批，避免内存泄漏。
    pub fn cleanup_expired(&self) {
        let timeout = Duration::from_secs(300);
        let now = Instant::now();
        let mut pending = self.pending.lock().unwrap();
        let expired: Vec<String> = pending
            .iter()
            .filter(|(_, p)| now.duration_since(p.created) > timeout)
            .map(|(id, _)| id.clone())
            .collect();
        for id in expired {
            pending.remove(&id);
            self.decisions.lock().unwrap().remove(&id);
        }
    }

    pub fn list_pending(&self) -> Vec<PendingApproval> {
        self.cleanup_expired();
        let mut items: Vec<PendingApproval> = self.pending.lock().unwrap().values().cloned().collect();
        items.sort_by_key(|p| p.created);
        items
    }

    pub fn respond(&self, id: &str, approved: bool) {
        self.pending.lock().unwrap().remove(id);
        self.decisions.lock().unwrap().insert(id.to_string(), approved);
    }

    /// 等待决策（轮询，最多 120 秒），返回是否允许。
    pub fn wait(&self, id: &str, timeout: Duration) -> bool {
        let start = Instant::now();
        loop {
            if let Some(decision) = self.decisions.lock().unwrap().get(id).copied() {
                self.decisions.lock().unwrap().remove(id);
                return decision;
            }
            if self.pending.lock().unwrap().get(id).is_none() {
                return false;
            }
            if start.elapsed() > timeout {
                self.pending.lock().unwrap().remove(id);
                return false;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}
