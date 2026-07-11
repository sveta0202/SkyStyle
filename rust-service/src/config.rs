use std::env;

#[derive(Clone)]
pub struct AppConfig {
    pub client: reqwest::Client,
    pub weather_key: String,
    pub llm_base_url: String,
    pub llm_key: String,
    pub llm_model: String,
    pub image_base_url: String,
    pub image_key: String,
    pub image_model: String,
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            client: reqwest::Client::new(),
            weather_key: required_env("WEATHER_API_KEY"),
            llm_base_url: env::var("LLM_BASE_URL")
                .unwrap_or_else(|_| "https://api.openai.com/v1".to_string()),
            llm_key: required_env("LLM_KEY"),
            llm_model: env::var("LLM_MODEL")
                .unwrap_or_else(|_| "gpt-4.1-mini".to_string()),
            image_base_url: env::var("IMAGE_BASE_URL")
                .unwrap_or_else(|_| "https://api.openai.com/v1".to_string()),
            image_key: required_env("IMAGE_API_KEY"),
            image_model: env::var("IMAGE_MODEL")
                .unwrap_or_else(|_| "gpt-image-1".to_string()),
        }
    }
}

fn required_env(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("missing required env var: {name}"))
}