use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EmbedError {
    #[error("FastEmbed error: {0}")]
    FastEmbed(String),
    #[error("Model load error: {0}")]
    ModelLoad(String),
    #[error("Generation error: {0}")]
    Generation(String),
}

pub trait Embedder: Send + Sync {
    fn embed_text(&self, text: &str) -> Result<Vec<f32>, EmbedError>;
    fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedError>;
    fn dimension(&self) -> usize;
    fn model_name(&self) -> &str;
}

pub struct FastEmbedder {
    model: TextEmbedding,
    model_name: String,
    dimension: usize,
}

impl FastEmbedder {
    pub fn try_new() -> Result<Self, EmbedError> {
        Self::try_new_with_cache_dir(None)
    }

    pub fn try_new_with_cache_dir(
        cache_dir: Option<std::path::PathBuf>,
    ) -> Result<Self, EmbedError> {
        Self::try_new_with_model_and_cache(EmbeddingModel::AllMiniLML6V2, cache_dir)
    }

    pub fn try_new_with_model(model_type: EmbeddingModel) -> Result<Self, EmbedError> {
        Self::try_new_with_model_and_cache(model_type, None)
    }

    pub fn try_new_with_model_and_cache(
        model_type: EmbeddingModel,
        cache_dir: Option<std::path::PathBuf>,
    ) -> Result<Self, EmbedError> {
        let (name, dim) = match model_type {
            EmbeddingModel::AllMiniLML6V2 => ("AllMiniLML6V2".to_string(), 384),
            EmbeddingModel::NomicEmbedTextV15 => ("NomicEmbedTextV15".to_string(), 768),
            _ => ("Custom".to_string(), 384),
        };

        let resolved_cache_dir = cache_dir.or_else(|| {
            std::env::var("FASTEMBED_CACHE_DIR")
                .ok()
                .map(std::path::PathBuf::from)
                .or_else(|| {
                    dirs::cache_dir().map(|p| p.join("serchx").join("models").join("fastembed"))
                })
        });

        let mut options = InitOptions::new(model_type);
        if let Some(ref dir) = resolved_cache_dir {
            if let Err(e) = std::fs::create_dir_all(dir) {
                tracing::warn!("FastEmbedder: failed to create cache_dir {:?}: {:?}", dir, e);
            }
            options = options.with_cache_dir(dir.clone());
        }

        let model =
            TextEmbedding::try_new(options).map_err(|e| EmbedError::ModelLoad(e.to_string()))?;

        Ok(Self {
            model,
            model_name: name,
            dimension: dim,
        })
    }
}

pub fn normalize_l2(vec: &mut [f32]) {
    let norm_sq: f32 = vec.iter().map(|&x| x * x).sum();
    if norm_sq > 0.0 {
        let norm = norm_sq.sqrt();
        for x in vec.iter_mut() {
            *x /= norm;
        }
    }
}

impl Embedder for FastEmbedder {
    fn embed_text(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
        let texts = vec![text];
        let mut results = self
            .model
            .embed(texts, None)
            .map_err(|e| EmbedError::Generation(e.to_string()))?;

        if let Some(mut vec) = results.pop() {
            normalize_l2(&mut vec);
            Ok(vec)
        } else {
            Err(EmbedError::Generation("No embedding returned".to_string()))
        }
    }

    fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let input: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
        let mut results = self
            .model
            .embed(input, None)
            .map_err(|e| EmbedError::Generation(e.to_string()))?;

        for vec in results.iter_mut() {
            normalize_l2(vec);
        }

        Ok(results)
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    fn model_name(&self) -> &str {
        &self.model_name
    }
}

#[derive(Clone)]
pub struct MockEmbedder {
    dimension: usize,
    name: String,
}

impl MockEmbedder {
    pub fn new(dimension: usize) -> Self {
        Self {
            dimension,
            name: "MockEmbedder".to_string(),
        }
    }
}

impl Default for MockEmbedder {
    fn default() -> Self {
        Self::new(384)
    }
}

impl Embedder for MockEmbedder {
    fn embed_text(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
        // Deterministic pseudo-embedding from text hash
        let mut vec = vec![0.0f32; self.dimension];
        let bytes = text.as_bytes();
        for (i, &b) in bytes.iter().enumerate() {
            vec[i % self.dimension] += (b as f32) / 255.0;
        }
        normalize_l2(&mut vec);
        Ok(vec)
    }

    fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedError> {
        texts.iter().map(|t| self.embed_text(t)).collect()
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    fn model_name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_embedder_dimension_and_normalization() {
        let embedder = MockEmbedder::new(384);
        assert_eq!(embedder.dimension(), 384);
        assert_eq!(embedder.model_name(), "MockEmbedder");

        let vec = embedder.embed_text("test document chunk").expect("embed");
        assert_eq!(vec.len(), 384);

        // Verify L2 norm is ~ 1.0
        let norm: f32 = vec.iter().map(|&x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_normalize_l2() {
        let mut v = vec![3.0, 4.0];
        normalize_l2(&mut v);
        assert!((v[0] - 0.6).abs() < 1e-5);
        assert!((v[1] - 0.8).abs() < 1e-5);
    }
}
