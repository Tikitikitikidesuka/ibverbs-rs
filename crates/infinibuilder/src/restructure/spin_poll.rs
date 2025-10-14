use std::time::{Duration, Instant};
use thiserror::Error;

/// Represents a timeout
#[derive(Debug, Copy, Clone, Error)]
#[error("Timeout: Duration {0:?} exceeded")]
pub struct Timeout(Duration);

/// Spins in a tight loop, repeatedly calling `event` until it returns `Some(_)`
/// or the given `timeout` duration has elapsed.
///
/// This function is useful for busy-wait polling in latency-critical code paths,
/// such as checking for RDMA completions or hardware events where blocking
/// or sleeping would add unwanted latency.
///
/// # Arguments
/// * `event` - A closure that returns `Some(value)` when the desired condition is met,
///   or `None` to continue polling.
/// * `timeout` - The maximum duration to spin before returning a [`Timeout`] error.
///
/// # Returns
/// * `Ok(value)` - If `event` returns `Some(value)` before the timeout expires.
/// * `Err(Timeout)` - If the timeout duration elapses before a result is available.
pub fn spin_poll<T>(
    mut event: impl FnMut() -> Option<T>,
    timeout: Duration,
) -> Result<T, Timeout> {
    let start_time = Instant::now();

    loop {
        let output = event();
        if let Some(output) = output {
            return Ok(output);
        }

        if start_time.elapsed() > timeout {
            return Err(Timeout(timeout));
        }
    }
}

/// Similar to [`spin_poll`], but checks the timeout only once every
/// `batch_iters` iterations to reduce timing overhead.
///
/// This can be more efficient when the `event` closure is expected to complete
/// quickly and the cost of calling [`Instant::elapsed`] on every iteration
/// is non-negligible.
///
/// # Arguments
/// * `event` - A closure that returns `Some(value)` when the desired condition is met,
///   or `None` to continue polling.
/// * `timeout` - The maximum duration to spin before returning a [`Timeout`] error.
/// * `batch_iters` - The number of polling iterations to perform before rechecking
///   the timeout.
///
/// # Returns
/// * `Ok(value)` - If `event` returns `Some(value)` before the timeout expires.
/// * `Err(Timeout)` - If the timeout duration elapses before a result is available.
pub fn spin_poll_batched<T>(
    mut event: impl FnMut() -> Option<T>,
    timeout: Duration,
    batch_iters: usize,
) -> Result<T, Timeout> {
    let start_time = Instant::now();
    let mut batch_counter = 0;

    loop {
        let output = event();
        if let Some(output) = output {
            return Ok(output);
        }

        if batch_counter >= batch_iters {
            if start_time.elapsed() > timeout {
                return Err(Timeout(timeout));
            }
            batch_counter = 0;
        } else {
            batch_counter += 1;
        }
    }
}