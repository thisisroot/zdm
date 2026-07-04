use std::time::Instant;

/// Turns a monotonically increasing byte counter into a smoothed throughput
/// reading, so the UI doesn't flicker between samples when one tick lands on a
/// slightly slower or faster window than its neighbors.
pub struct SpeedTracker {
    last_sample_at: Instant,
    last_bytes: u64,
    smoothed_bps: f64,
}

impl SpeedTracker {
    pub fn new() -> Self {
        Self { last_sample_at: Instant::now(), last_bytes: 0, smoothed_bps: 0.0 }
    }

    /// Call periodically with the cumulative bytes downloaded so far.
    pub fn sample(&mut self, total_bytes: u64) -> f64 {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_sample_at).as_secs_f64();
        if elapsed > 0.0 {
            let delta = total_bytes.saturating_sub(self.last_bytes) as f64;
            let instantaneous = delta / elapsed;
            // Exponential moving average: recent samples dominate, but one stalled
            // tick doesn't make the readout jump to zero and immediately back.
            const ALPHA: f64 = 0.35;
            self.smoothed_bps = if self.smoothed_bps == 0.0 {
                instantaneous
            } else {
                ALPHA * instantaneous + (1.0 - ALPHA) * self.smoothed_bps
            };
        }
        self.last_sample_at = now;
        self.last_bytes = total_bytes;
        self.smoothed_bps
    }
}

impl Default for SpeedTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn converges_toward_a_steady_rate() {
        let mut tracker = SpeedTracker::new();
        let mut total = 0u64;
        let mut last = 0.0;
        for _ in 0..20 {
            sleep(Duration::from_millis(20));
            total += 20_000; // ~1 MB/s
            last = tracker.sample(total);
        }
        assert!((last - 1_000_000.0).abs() / 1_000_000.0 < 0.25, "expected ~1MB/s, got {last}");
    }
}
