use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, Default)]
pub struct CrawlController {
    pause_flag: Arc<AtomicBool>,
    stop_flag: Arc<AtomicBool>,
}

impl CrawlController {
    pub fn new() -> Self {
        Self {
            pause_flag: Arc::new(AtomicBool::new(false)),
            stop_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn pause(&self) {
        self.pause_flag.store(true, Ordering::SeqCst);
    }

    pub fn resume(&self) {
        self.pause_flag.store(false, Ordering::SeqCst);
    }

    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::SeqCst);
    }

    pub fn is_paused(&self) -> bool {
        self.pause_flag.load(Ordering::SeqCst)
    }

    pub fn is_stopped(&self) -> bool {
        self.stop_flag.load(Ordering::SeqCst)
    }

    pub fn reset_stop(&self) {
        self.stop_flag.store(false, Ordering::SeqCst);
    }

    pub fn reset(&self) {
        self.pause_flag.store(false, Ordering::SeqCst);
        self.stop_flag.store(false, Ordering::SeqCst);
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_crawl_controller_flags() {
        let controller = CrawlController::new();
        assert!(!controller.is_paused());
        assert!(!controller.is_stopped());

        controller.pause();
        assert!(controller.is_paused());

        controller.resume();
        assert!(!controller.is_paused());

        controller.stop();
        assert!(controller.is_stopped());

        controller.reset();
        assert!(!controller.is_paused());
        assert!(!controller.is_stopped());
    }
}
