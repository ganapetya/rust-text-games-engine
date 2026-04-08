use shakti_game_infrastructure::LlmMode;

/// Resolved OpenAI credential source (structured logs only; never log the key).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenAiKeySource {
    None,
    Env,
    File,
    /// Decrypted via `GET /api/keys/get/{key}/{service}` on shakti-actors (Shakti `api_keys` table).
    Actors,
}

impl OpenAiKeySource {
    pub fn as_str(self) -> &'static str {
        match self {
            OpenAiKeySource::None => "none",
            OpenAiKeySource::Env => "env",
            OpenAiKeySource::File => "file",
            OpenAiKeySource::Actors => "actors",
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
    /// When set, OpenAI key is fetched from shakti-actors (lazy, on first LLM call).
    pub shakti_actors_internal_url: Option<String>,
    pub shakti_actors_openai_key_name: String,
    pub shakti_actors_openai_consumer_service: String,
    /// When true, session API includes per-step correct gap words for local/dev testing (do not enable in production).
    pub dev_expose_gap_solution: bool,
    /// Shared secret for server-to-server `POST /api/v1/game-sessions/bootstrap` only.
    pub service_api_key: Option<String>,
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

        let (openai_api_key, mut openai_key_source) = resolve_openai_key();

        let shakti_actors_internal_url = std::env::var("SHAKTI_ACTORS_INTERNAL_URL")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let shakti_actors_openai_key_name = std::env::var("SHAKTI_ACTORS_OPENAI_KEY_NAME")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "openai_main".into());

        let shakti_actors_openai_consumer_service =
            std::env::var("SHAKTI_ACTORS_OPENAI_CONSUMER_SERVICE")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "shakti-game-engine".into());

        let llm_mode = match std::env::var("GAME_ENGINE_LLM_MODE")
            .ok()
            .as_deref()
            .map(str::trim)
        {
            None | Some("") => {
                if openai_api_key.is_some() {
                    LlmMode::OpenAi
                } else if shakti_actors_internal_url.is_some() {
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

        let service_api_key = std::env::var("GAME_ENGINE_SERVICE_API_KEY")
            .ok()
            .and_then(|s| trim_key(&s));

        if llm_mode == LlmMode::OpenAi
            && openai_api_key.is_none()
            && shakti_actors_internal_url.is_some()
        {
            openai_key_source = OpenAiKeySource::Actors;
        }

        Ok(Config {
            database_url,
            app_port,
            llm_mode,
            openai_api_key,
            openai_model,
            openai_key_source,
            shakti_actors_internal_url,
            shakti_actors_openai_key_name,
            shakti_actors_openai_consumer_service,
            dev_expose_gap_solution,
            service_api_key,
        })
    }
}
