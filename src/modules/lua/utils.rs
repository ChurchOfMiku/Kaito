use once_cell::sync::OnceCell;
use std::time::Instant;

static INSTANT: OnceCell<Instant> = OnceCell::new();

pub fn get_duration() -> f64 {
    let elapsed = INSTANT.get_or_init(|| Instant::now()).elapsed();

    elapsed.as_secs() as f64 * 1_000_000.0 + elapsed.subsec_nanos() as f64 / 1_000.0
}
