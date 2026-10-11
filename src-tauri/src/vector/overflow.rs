#[derive(Debug, Default, Clone)]
pub struct CorpusCapManager;

impl CorpusCapManager {
    pub fn new() -> Self {
        Self
    }

    /// Evaluates whether a chunk should receive a vector embedding.
    /// Pinned items are always protected and embedded regardless of cap.
    /// Unpinned chunks are rejected once total_vectors reaches or exceeds max_vectors.
    pub fn can_embed_chunk(
        &self,
        is_pinned: bool,
        total_vectors: usize,
        max_vectors: usize,
    ) -> bool {
        if is_pinned {
            return true;
        }
        total_vectors < max_vectors
    }

    /// Evaluates whether a new file can be indexed under the file cap.
    pub fn can_index_file(&self, total_files: usize, max_files: usize) -> bool {
        total_files < max_files
    }

    /// Generates user-facing warning messages when vector corpus cap is approached or reached.
    pub fn should_warn_overflow(&self, total_vectors: usize, max_vectors: usize) -> Option<String> {
        if max_vectors == 0 {
            return None;
        }

        if total_vectors >= max_vectors {
            Some(format!(
                "Vector corpus cap reached ({}/{} chunks). New unpinned files will degrade gracefully to keyword FTS search.",
                total_vectors, max_vectors
            ))
        } else if (total_vectors as f64) >= (max_vectors as f64) * 0.9 {
            Some(format!(
                "Vector corpus approaching cap ({}/{} chunks, >90% used).",
                total_vectors, max_vectors
            ))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_corpus_cap_manager_chunk_embedding() {
        let manager = CorpusCapManager::new();
        let cap = 1000;

        // Under cap: unpinned and pinned allowed
        assert!(manager.can_embed_chunk(false, 500, cap));
        assert!(manager.can_embed_chunk(true, 500, cap));

        // At or above cap: unpinned rejected, pinned still allowed
        assert!(!manager.can_embed_chunk(false, 1000, cap));
        assert!(manager.can_embed_chunk(true, 1000, cap));
        assert!(!manager.can_embed_chunk(false, 1050, cap));
        assert!(manager.can_embed_chunk(true, 1050, cap));
    }

    #[test]
    fn test_corpus_cap_manager_file_cap() {
        let manager = CorpusCapManager::new();
        assert!(manager.can_index_file(100, 20_000));
        assert!(!manager.can_index_file(20_000, 20_000));
        assert!(!manager.can_index_file(20_001, 20_000));
    }

    #[test]
    fn test_corpus_cap_manager_warning() {
        let manager = CorpusCapManager::new();
        let cap = 1000;

        // Under 90%: no warning
        assert!(manager.should_warn_overflow(800, cap).is_none());

        // At 90%: approaching cap warning
        let warn_90 = manager.should_warn_overflow(900, cap);
        assert!(warn_90.is_some());
        assert!(warn_90.unwrap().contains(">90%"));

        // At or above 100%: cap reached warning
        let warn_full = manager.should_warn_overflow(1000, cap);
        assert!(warn_full.is_some());
        assert!(warn_full.unwrap().contains("Vector corpus cap reached"));
    }
}
