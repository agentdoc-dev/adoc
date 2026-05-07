#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ModelId {
    pub(crate) id: String,
    pub(crate) provider: String,
}

impl ModelId {
    pub(crate) fn new(id: impl Into<String>, provider: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            provider: provider.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EmbeddingError {
    ModelLoad(String),
    Compute(String),
    #[allow(dead_code)]
    DimensionMismatch {
        expected: usize,
        actual: usize,
    },
}

pub(crate) trait EmbeddingProvider {
    fn model_id(&self) -> &ModelId;
    fn dim(&self) -> usize;
    fn embed_passages(&self, inputs: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError>;
    #[allow(dead_code)]
    fn embed_query(&self, query: &str) -> Result<Vec<f32>, EmbeddingError>;
}
