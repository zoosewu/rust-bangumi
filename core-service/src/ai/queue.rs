//! 全域 AI 請求佇列：限制同時送出的 AI 請求數（設定頁可調，預設 1）。
//!
//! 所有 AI 生成共用同一個 provider 額度（例如每分鐘 token 數），同時送出太多請求
//! 會一起撞上限流，所以整個 process 只有一條佇列。

use std::sync::{Mutex, OnceLock};
use tokio::sync::Notify;

pub const DEFAULT_CONCURRENCY: usize = 1;
pub const MAX_CONCURRENCY: usize = 10;

struct Slots {
    limit: usize,
    running: usize,
}

pub struct AiQueue {
    slots: Mutex<Slots>,
    released: Notify,
}

/// 佔用一個併行名額，drop 時歸還。
pub struct AiPermit<'a> {
    queue: &'a AiQueue,
}

impl AiQueue {
    pub fn new(limit: usize) -> Self {
        Self {
            slots: Mutex::new(Slots {
                limit: limit.clamp(1, MAX_CONCURRENCY),
                running: 0,
            }),
            released: Notify::new(),
        }
    }

    pub fn limit(&self) -> usize {
        self.slots.lock().unwrap().limit
    }

    /// 調整併行數。調高時立刻放行排隊中的請求；調低時已在跑的請求不受影響，
    /// 之後的請求等到執行中的數量低於新上限才放行。
    pub fn set_limit(&self, limit: usize) {
        self.slots.lock().unwrap().limit = limit.clamp(1, MAX_CONCURRENCY);
        self.released.notify_waiters();
    }

    pub async fn acquire(&self) -> AiPermit<'_> {
        loop {
            // 先登記等待再檢查名額，避免檢查後、等待前的歸還被漏掉
            let released = self.released.notified();
            tokio::pin!(released);
            released.as_mut().enable();
            if self.try_take() {
                return AiPermit { queue: self };
            }
            released.await;
        }
    }

    fn try_take(&self) -> bool {
        let mut slots = self.slots.lock().unwrap();
        let free = slots.running < slots.limit;
        if free {
            slots.running += 1;
        }
        free
    }
}

impl Drop for AiPermit<'_> {
    fn drop(&mut self) {
        self.queue.slots.lock().unwrap().running -= 1;
        self.queue.released.notify_waiters();
    }
}

/// 整個 process 共用的 AI 佇列；啟動時與設定變更時以 `set_limit` 套用設定值。
pub fn ai_queue() -> &'static AiQueue {
    static QUEUE: OnceLock<AiQueue> = OnceLock::new();
    QUEUE.get_or_init(|| AiQueue::new(DEFAULT_CONCURRENCY))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::timeout;

    const SHORT: Duration = Duration::from_millis(50);

    #[tokio::test]
    async fn waits_until_a_running_request_finishes() {
        let queue = AiQueue::new(1);
        let first = queue.acquire().await;
        assert!(timeout(SHORT, queue.acquire()).await.is_err());

        drop(first);
        assert!(timeout(SHORT, queue.acquire()).await.is_ok());
    }

    #[tokio::test]
    async fn runs_up_to_the_limit_at_once() {
        let queue = AiQueue::new(2);
        let _a = queue.acquire().await;
        let _b = queue.acquire().await;
        assert!(timeout(SHORT, queue.acquire()).await.is_err());
    }

    #[tokio::test]
    async fn raising_the_limit_releases_a_waiting_request() {
        let queue = std::sync::Arc::new(AiQueue::new(1));
        let _running = queue.acquire().await;

        let waiting = {
            let queue = queue.clone();
            tokio::spawn(async move {
                let _permit = queue.acquire().await;
            })
        };
        tokio::time::sleep(SHORT).await;
        assert!(!waiting.is_finished());

        queue.set_limit(2);
        assert!(timeout(SHORT, waiting).await.is_ok());
    }

    #[tokio::test]
    async fn lowering_the_limit_holds_new_requests_until_below_it() {
        let queue = AiQueue::new(2);
        let a = queue.acquire().await;
        let b = queue.acquire().await;
        queue.set_limit(1);

        drop(a);
        assert!(timeout(SHORT, queue.acquire()).await.is_err());
        drop(b);
        assert!(timeout(SHORT, queue.acquire()).await.is_ok());
    }

    #[test]
    fn limit_is_clamped() {
        assert_eq!(AiQueue::new(0).limit(), 1);
        assert_eq!(AiQueue::new(99).limit(), MAX_CONCURRENCY);
    }
}
