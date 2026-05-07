use crate::domain::ports::embedding_provider::{EmbeddingError, EmbeddingProvider, ModelId};

#[derive(Debug, Clone)]
pub(crate) struct InMemoryProvider {
    model_id: ModelId,
    dim: usize,
}

impl InMemoryProvider {
    pub(crate) fn new(dim: usize) -> Self {
        Self {
            model_id: ModelId::new("in-memory", "test"),
            dim,
        }
    }

    fn embed_text(&self, text: &str) -> Vec<f32> {
        (0..self.dim)
            .map(|index| stable_component(text.as_bytes(), index as u64))
            .collect()
    }
}

impl EmbeddingProvider for InMemoryProvider {
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
    use crate::infrastructure::embedding::in_memory::InMemoryProvider;

    #[test]
    fn in_memory_provider_returns_deterministic_vectors_with_configured_dim() {
        let provider = InMemoryProvider::new(4);

        let first = provider
            .embed_passages(&["Credits apply after payment.".to_string()])
            .expect("passage embedding succeeds");
        let second = provider
            .embed_passages(&["Credits apply after payment.".to_string()])
            .expect("passage embedding succeeds");

        assert_eq!(provider.model_id().id, "in-memory");
        assert_eq!(provider.model_id().provider, "test");
        assert_eq!(provider.dim(), 4);
        assert_eq!(first, second);
        assert_eq!(first[0].len(), 4);
        assert_ne!(first[0], vec![0.0; 4]);
    }
}
