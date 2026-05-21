use crate::domain::ports::embedding_provider::{EmbeddingError, EmbeddingProvider, ModelId};

pub(crate) const MODEL_ID: &str = "hash-v1";
pub(crate) const PROVIDER_ID: &str = "deterministic";
pub(crate) const DEFAULT_DIM: usize = 384;

#[derive(Debug, Clone)]
pub(crate) struct DeterministicProvider {
    model_id: ModelId,
    dim: usize,
}

impl DeterministicProvider {
    pub(crate) fn default() -> Self {
        Self::new(DEFAULT_DIM)
    }

    pub(crate) fn new(dim: usize) -> Self {
        Self {
            model_id: ModelId::new(MODEL_ID, PROVIDER_ID),
            dim,
        }
    }

    /// Returns the deterministic provider's `SearchModelHeader` without
    /// constructing an instance.
    #[allow(dead_code)]
    pub(crate) fn metadata_header() -> crate::domain::artifact::SearchModelHeader {
        crate::domain::artifact::SearchModelHeader {
            id: MODEL_ID.to_string(),
            provider: PROVIDER_ID.to_string(),
            dim: DEFAULT_DIM,
        }
    }

    fn embed_text(&self, text: &str) -> Vec<f32> {
        (0..self.dim)
            .map(|index| stable_component(text.as_bytes(), index as u64))
            .collect()
    }
}

impl EmbeddingProvider for DeterministicProvider {
    fn model_id(&self) -> &ModelId {
        &self.model_id
    }

    fn dim(&self) -> usize {
        self.dim
    }

    fn embed_passages(&self, inputs: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        Ok(inputs.iter().map(|input| self.embed_text(input)).collect())
    }

    fn embed_query(&self, query: &str) -> Result<Vec<f32>, EmbeddingError> {
        Ok(self.embed_text(query))
    }
}

fn stable_component(bytes: &[u8], seed: u64) -> f32 {
    let mut hash = 0xcbf29ce484222325_u64 ^ seed.wrapping_mul(0x100000001b3);
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    let bucket = (hash % 2001) as f32;
    (bucket - 1000.0) / 1000.0
}

#[cfg(test)]
mod tests {
    use crate::domain::ports::embedding_provider::EmbeddingProvider;
    use crate::infrastructure::embedding::deterministic::DeterministicProvider;

    #[test]
    fn deterministic_provider_returns_repeatable_vectors_with_configured_dim() {
        let provider = DeterministicProvider::new(4);

        let first = provider
            .embed_passages(&["Credits apply after payment.".to_string()])
            .expect("passage embedding succeeds");
        let second = provider
            .embed_passages(&["Credits apply after payment.".to_string()])
            .expect("passage embedding succeeds");

        assert_eq!(provider.model_id().id, "hash-v1");
        assert_eq!(provider.model_id().provider, "deterministic");
        assert_eq!(provider.dim(), 4);
        assert_eq!(first, second);
        assert_eq!(first[0].len(), 4);
        assert_ne!(first[0], vec![0.0; 4]);
    }
}
