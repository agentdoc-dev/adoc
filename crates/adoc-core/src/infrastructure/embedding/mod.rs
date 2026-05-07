pub(crate) mod fastembed;
#[cfg(any(test, feature = "test-embedding-provider"))]
pub(crate) mod in_memory;

#[cfg(test)]
mod tests {
    use crate::domain::ports::embedding_provider::EmbeddingProvider;
    use crate::infrastructure::embedding::fastembed::FastEmbedProvider;

    #[test]
    fn fastembed_provider_header_matches_search_contract_without_model_download() {
        let provider = FastEmbedProvider::metadata_only_for_test();

        assert_eq!(provider.model_id().id, "bge-small-en-v1.5");
        assert_eq!(provider.model_id().provider, "fastembed");
        assert_eq!(provider.dim(), 384);
    }
}
