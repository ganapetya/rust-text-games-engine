use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Score {
    pub total_points: i32,
    pub earned_points: i32,
    pub correct_count: i32,
    pub answered_count: i32,
}

impl Score {
    pub fn accuracy(&self) -> f32 {
        if self.answered_count == 0 {
            return 0.0;
        }
        self.correct_count as f32 / self.answered_count as f32
    }
}
