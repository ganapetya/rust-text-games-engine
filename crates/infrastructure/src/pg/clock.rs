use shakti_game_engine_core::Clock;
use time::OffsetDateTime;

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }
}
