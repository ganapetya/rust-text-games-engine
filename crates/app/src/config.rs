use shakti_game_infrastructure::LlmMode;

/// Resolved OpenAI credential source (structured logs only; never log the key).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenAiKeySource {
    None,
    Env,
    File,
}

impl OpenAiKeySource {
    pub fn as_str(self) -> &'static str {
        match self {
            OpenAiKeySource::None => "none",
            OpenAiKeySource::Env => "env",
            OpenAiKeySource::File => "file",
        }
    }
}

pub struct Config {
    pub database_url: String,
    pub app_port: u16,
    pub llm_mode: LlmMode,
    pub openai_api_key: Option<String>,
    pub openai_model: String,
    pub openai_key_source: OpenAiKeySource,
    /// When true, session API includes per-step correct gap words for local/dev testing (do not enable in production).
    pub dev_expose_gap_solution: bool,
}

fn env_truthy(var: &str) -> bool {
    std::env::var(var)
        .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false)
}

fn trim_key(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

fn resolve_openai_key() -> (Option<String>, OpenAiKeySource) {
    if let Ok(v) = std::env::var("OPENAI_API_KEY") {
        if let Some(k) = trim_key(&v) {
            return (Some(k), OpenAiKeySource::Env);
        }
    }

    let path = std::env::var("OPENAI_KEY_FILE").unwrap_or_else(|_| "openai.key.secret".to_string());
    match std::fs::read_to_string(&path) {
        Ok(contents) => match trim_key(&contents) {
            Some(k) => (Some(k), OpenAiKeySource::File),
            None => (None, OpenAiKeySource::None),
        },
        Err(_) => (None, OpenAiKeySource::None),
    }
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let database_url =
            std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL must be set".to_string())?;
        let app_port = std::env::var("APP_PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(8010);

        let (openai_api_key, openai_key_source) = resolve_openai_key();

        let llm_mode = match std::env::var("GAME_ENGINE_LLM_MODE")
            .ok()
            .as_deref()
            .map(str::trim)
        {
            None | Some("") => {
                if openai_api_key.is_some() {
                    LlmMode::OpenAi
                } else {
                    LlmMode::Mock
                }
            }
            Some("mock") => LlmMode::Mock,
            Some("openai") => LlmMode::OpenAi,
            Some(other) => {
                return Err(format!(
                    "GAME_ENGINE_LLM_MODE: invalid value {other:?} (use mock or openai)"
                ));
            }
        };

        let openai_model = std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-5.2".into());
        let dev_expose_gap_solution = env_truthy("GAME_ENGINE_DEV_EXPOSE_GAP_SOLUTION");

        Ok(Config {
            database_url,
            app_port,
            llm_mode,
            openai_api_key,
            openai_model,
            openai_key_source,
            dev_expose_gap_solution,
        })
    }
}
