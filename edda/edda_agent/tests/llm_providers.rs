use edda_agent::llm::*;

#[tokio::test]
async fn test_anthropic_text() {
    test_llm_text_impl("test_anthropic_text", LLMProvider::Anthropic).await
}

#[tokio::test]
async fn test_gemini_text() {
    test_llm_text_impl("test_gemini_text", LLMProvider::Gemini).await
}

#[tokio::test]
async fn test_openrouter_text() {
    test_llm_text_impl("test_openrouter_text", LLMProvider::OpenRouter).await
}

async fn test_llm_text_impl(test_name: &str, llm_provider: LLMProvider) {
    dotenvy::dotenv().ok();
    if !llm_provider.is_api_key_env_var_set() {
        eprintln!(
            "Skipping {test_name}: env var {} not set",
            llm_provider.api_key_env_var()
        );
        return;
    }

    let client = llm_provider.client_from_env_raw();
    let completion = Completion::new(
        llm_provider.default_model().to_string(),
        rig::message::Message::user("say hi"),
    )
    .max_tokens(256);
    let response = LLMClient::completion(&client, completion).await;
    assert!(response.is_ok());
}
