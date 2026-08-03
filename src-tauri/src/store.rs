use crate::database::Database;
use crate::services::{ProxyService, UsageCache};
use std::sync::Arc;

/// 全局应用状态
#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Database>,
    pub proxy_service: ProxyService,
    pub usage_cache: Arc<UsageCache>,
    /// Serializes an entire Profile application across UI, tray and deep links.
    pub profile_apply_lock: Arc<tokio::sync::Mutex<()>>,
}

impl AppState {
    /// 创建新的应用状态
    pub fn new(db: Arc<Database>) -> Self {
        let proxy_service = ProxyService::new(db.clone());

        Self {
            db,
            proxy_service,
            usage_cache: Arc::new(UsageCache::new()),
            profile_apply_lock: Arc::new(tokio::sync::Mutex::new(())),
        }
    }
}
