use std::sync::Mutex;

use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

use crate::domain::ports::embedding_provider::{EmbeddingError, EmbeddingProvider, ModelId};

const MODEL_ID: &str = "bge-small-en-v1.5";
const PROVIDER_ID: &str = "fastembed";
const DIM: usize = 384;
const QUERY_INSTRUCTION: &str = "Represent this sentence for searching relevant passages: ";

pub(crate) struct FastEmbedProvider {
    model_id: ModelId,
    model: Option<Mutex<TextEmbedding>>,
}

impl FastEmbedProvider {
    pub(crate) fn try_new() -> Result<Self, EmbeddingError> {
        let model = TextEmbedding::try_new(InitOptions::new(EmbeddingModel::BGESmallENV15))
            .map_err(|error| EmbeddingError::ModelLoad(error.to_string()))?;

        Ok(Self {
            model_id: Self::default_model_id(),
            model: Some(Mutex::new(model)),
        })
    }

    fn default_model_id() -> ModelId {
        ModelId::new(MODEL_ID, PROVIDER_ID)
    }

    /// Returns the active provider's `SearchModelHeader` without loading the
    /// underlying model. Used by `load_retrieval_session` to validate a search
    /// artifact's model header against the configured provider without paying
    /// the model-download cost.
    pub(crate) fn metadata_header() -> crate::domain::artifact::SearchModelHeader {
        crate::domain::artifact::SearchModelHeader {
            id: MODEL_ID.to_string(),
            provider: PROVIDER_ID.to_string(),
            dim: DIM,
        }
    }

    #[cfg(test)]
    pub(crate) fn metadata_only_for_test() -> Self {
        Self {
            model_id: Self::default_model_id(),
            model: None,
        }
    }

    fn embed_texts<S>(&self, texts: S) -> Result<Vec<Vec<f32>>, EmbeddingError>
    where
        S: AsRef<[String]>,
    {
        let model = self
            .model
            .as_ref()
            .ok_or_else(|| EmbeddingError::Compute("FastEmbed model is not loaded".to_string()))?;
        let mut model = model.lock().map_err(|error| {
            EmbeddingError::Compute(format!("FastEmbed model lock failed: {error}"))
        })?;

        model
            .embed(texts, None)
            .map_err(|error| EmbeddingError::Compute(error.to_string()))
    }

    fn validate_vector_dimensions(
        &self,
        vectors: Vec<Vec<f32>>,
    ) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        for vector in &vectors {
            if vector.len() != DIM {
                return Err(EmbeddingError::DimensionMismatch {
                    expected: DIM,
                    actual: vector.len(),
                });
            }
        }

        Ok(vectors)
    }
}

impl EmbeddingProvider for FastEmbedProvider {
    fn model_id(&self) -> &ModelId {
        &self.model_id
    }

    fn dim(&self) -> usize {
        DIM
    }

    fn embed_passages(&self, inputs: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        let vectors = self.embed_texts(inputs)?;
        self.validate_vector_dimensions(vectors)
    }

    fn embed_query(&self, query: &str) -> Result<Vec<f32>, EmbeddingError> {
        let query = format_query_for_passage_search(query);
        let vectors = self.validate_vector_dimensions(self.embed_texts(vec![query])?)?;
        query_vector_from_fastembed_response(vectors)
    }
}

fn format_query_for_passage_search(query: &str) -> String {
    format!("{QUERY_INSTRUCTION}{query}")
}

fn query_vector_from_fastembed_response(
    vectors: Vec<Vec<f32>>,
) -> Result<Vec<f32>, EmbeddingError> {
    vectors
        .into_iter()
        .next()
        .ok_or_else(|| EmbeddingError::Compute("fastembed returned no vectors".to_string()))
}

#[cfg(all(test, feature = "fastembed-it"))]
mod fastembed_it_tests {
    use crate::domain::ports::embedding_provider::EmbeddingProvider;
    use crate::infrastructure::embedding::fastembed::FastEmbedProvider;

    #[test]
    fn fastembed_provider_embeds_passage_end_to_end() {
        let provider = FastEmbedProvider::try_new().expect("FastEmbed model loads");

        let vectors = provider
            .embed_passages(&["claim: Credits apply after payment.".to_string()])
            .expect("FastEmbed computes passage embedding");

        assert_eq!(vectors.len(), 1);
        assert_eq!(vectors[0].len(), provider.dim());
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::ports::embedding_provider::EmbeddingError;
    use crate::infrastructure::embedding::fastembed::{
        format_query_for_passage_search, query_vector_from_fastembed_response,
    };

    #[test]
    fn format_query_for_passage_search_uses_bge_search_instruction() {
        assert_eq!(
            format_query_for_passage_search("refund policy"),
            "Represent this sentence for searching relevant passages: refund policy"
        );
    }

    #[test]
    fn query_vector_from_fastembed_response_maps_empty_response_to_compute_error() {
        let error = query_vector_from_fastembed_response(Vec::new()).expect_err("empty response");

        assert_eq!(
            error,
            EmbeddingError::Compute("fastembed returned no vectors".to_string())
        );
    }
}
