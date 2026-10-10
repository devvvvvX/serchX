use crate::vector::embedder::{EmbedError, Embedder, FastEmbedder};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

type EmbedderFactory = Box<dyn Fn() -> Result<Arc<dyn Embedder>, EmbedError> + Send + Sync>;

pub struct ModelScheduler {
    model: Arc<Mutex<Option<Arc<dyn Embedder>>>>,
    last_used: Arc<AtomicI64>,
    idle_timeout_secs: i64,
    factory: EmbedderFactory,
}

impl ModelScheduler {
    pub fn new(idle_timeout_secs: i64) -> Self {
        Self::with_factory(idle_timeout_secs, || {
            let embedder = FastEmbedder::try_new()?;
            Ok(Arc::new(embedder) as Arc<dyn Embedder>)
        })
    }

    pub fn with_factory<F>(idle_timeout_secs: i64, factory: F) -> Self
    where
        F: Fn() -> Result<Arc<dyn Embedder>, EmbedError> + Send + Sync + 'static,
    {
        Self {
            model: Arc::new(Mutex::new(None)),
            last_used: Arc::new(AtomicI64::new(0)),
            idle_timeout_secs,
            factory: Box::new(factory),
        }
    }

    pub async fn get_model(&self) -> Result<Arc<dyn Embedder>, EmbedError> {
        let mut guard = self.model.lock().await;
        let now = chrono::Utc::now().timestamp();
        self.last_used.store(now, Ordering::Relaxed);

        if let Some(ref m) = *guard {
            return Ok(m.clone());
        }

        let new_model = (self.factory)()?;
        *guard = Some(new_model.clone());
        Ok(new_model)
    }

    pub async fn is_loaded(&self) -> bool {
        let guard = self.model.lock().await;
        guard.is_some()
    }

    pub async fn unload_if_idle(&self) -> bool {
        let now = chrono::Utc::now().timestamp();
        let last = self.last_used.load(Ordering::Relaxed);

        if last > 0 && now - last >= self.idle_timeout_secs {
            let mut guard = self.model.lock().await;
            if guard.is_some() {
                *guard = None;
                return true;
            }
        }
        false
    }

    pub async fn force_unload(&self) {
        let mut guard = self.model.lock().await;
        *guard = None;
    }

    pub fn last_used_timestamp(&self) -> i64 {
        self.last_used.load(Ordering::Relaxed)
    }

    pub fn idle_timeout_secs(&self) -> i64 {
        self.idle_timeout_secs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vector::embedder::MockEmbedder;

    #[tokio::test]
    async fn test_model_scheduler_lifecycle() {
        let scheduler = ModelScheduler::with_factory(2, || {
            Ok(Arc::new(MockEmbedder::new(384)) as Arc<dyn Embedder>)
        });

        // Initially not loaded
        assert!(!scheduler.is_loaded().await);

        // First call loads model
        let model = scheduler.get_model().await.expect("get model");
        assert_eq!(model.dimension(), 384);
        assert!(scheduler.is_loaded().await);

        // Unload immediately: not idle yet (timeout is 2s)
        let unloaded = scheduler.unload_if_idle().await;
        assert!(!unloaded);
        assert!(scheduler.is_loaded().await);

        // Force unload
        scheduler.force_unload().await;
        assert!(!scheduler.is_loaded().await);

        // Simulate idle timeout by setting last_used in the past
        let _ = scheduler.get_model().await.expect("get model again");
        assert!(scheduler.is_loaded().await);
        scheduler
            .last_used
            .store(chrono::Utc::now().timestamp() - 10, Ordering::Relaxed);

        let idle_unloaded = scheduler.unload_if_idle().await;
        assert!(idle_unloaded);
        assert!(!scheduler.is_loaded().await);
    }
}
