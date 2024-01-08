use std::time::SystemTime;

pub trait Monitor {
    fn init() -> Self;
    fn on_loop(&mut self, num_iters: u64);
}

pub struct NullMonitor;
pub struct IterationMonitor<const N: u64> {
    last_time: SystemTime,
    last_report: u64,
}

impl Monitor for NullMonitor {
    #[inline]
    fn init() -> Self {
        Self
    }
    #[inline]
    fn on_loop(&mut self, _num_iters: u64) {}
}

impl<const N: u64> Monitor for IterationMonitor<N> {
    fn init() -> Self {
        Self {
            last_time: std::time::SystemTime::now(),
            last_report: 0,
        }
    }

    fn on_loop(&mut self, num_iters: u64) {
        let delta_iters = num_iters - self.last_report;
        if delta_iters > N {
            let delta_time = self.last_time.elapsed().unwrap().as_secs_f64();
            let nanos_per_fuzz = (delta_time * 1_000_000.) / (delta_iters as f64);

            let (units_per_fuzz, unit) = if nanos_per_fuzz > 1_000_000. {
                (nanos_per_fuzz / 1_000_000., "s")
            } else if nanos_per_fuzz > 1_000. {
                (nanos_per_fuzz / 1_000., "ms")
            } else {
                (nanos_per_fuzz, "ns")
            };

            println!("[MONITOR]: {num_iters} iterations done ({units_per_fuzz:.03}{unit} / fuzz over last {delta_iters})");

            self.last_report = num_iters;
            self.last_time = SystemTime::now();
        }
    }
}
