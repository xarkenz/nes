use std::time::{Duration, Instant};

pub struct FrameTimer {
    period: Duration,
    last_target: Instant,
}

impl FrameTimer {
    pub fn start(frames_per_second: f64) -> Self {
        Self {
            period: Duration::from_secs_f64(1.0 / frames_per_second),
            last_target: Instant::now(),
        }
    }

    pub fn period(&self) -> Duration {
        self.period
    }

    pub fn set_period(&mut self, period: Duration) {
        self.period = period;
    }

    pub fn set_frequency(&mut self, frames_per_second: f64) {
        self.period = Duration::from_secs_f64(1.0 / frames_per_second);
    }

    pub fn last_target(&self) -> Instant {
        self.last_target
    }

    pub fn target(&self) -> Instant {
        self.last_target + self.period
    }

    pub fn wait_until_target(&mut self) {
        let target = self.target();
        if let Some(remaining_time) = target.checked_duration_since(Instant::now()) {
            std::thread::sleep(remaining_time);
        }
        self.last_target = target;
    }
}
