use shakti_game_infrastructure::LlmMode;

pub struct Config {
    pub database_url: String,
    pub app_port: u16,
    pub llm_mode: LlmMode,
    pub openai_api_key: Option<String>,
    pub openai_model: String,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let database_url =
            std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL must be set".to_string())?;
        let app_port = std::env::var("APP_PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(8010);

        let llm_mode = std::env::var("GAME_ENGINE_LLM_MODE")
            .ok()
            .and_then(|s| LlmMode::parse(&s))
            .unwrap_or(LlmMode::Mock);

        let openai_api_key = std::env::var("OPENAI_API_KEY").ok();
        let openai_model = std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".into());

        Ok(Config {
            database_url,
            app_port,
            llm_mode,
            openai_api_key,
            openai_model,
        })
    }
}
