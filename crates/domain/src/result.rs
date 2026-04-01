use crate::score::Score;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameResult {
    pub score: Score,
    pub summary: String,
}
