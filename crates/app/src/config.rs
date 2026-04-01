pub struct Config {
    pub database_url: String,
    pub app_port: u16,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let database_url =
            std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL must be set".to_string())?;
        let app_port = std::env::var("APP_PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(8010);
        Ok(Config {
            database_url,
            app_port,
        })
    }
}
