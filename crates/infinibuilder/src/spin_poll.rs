use std::time::{Duration, Instant};

/// Spins in a tight loop, repeatedly calling `event` until it returns `Ok(_)`
/// or the given `timeout` duration has elapsed.
///
/// This function is useful for busy-wait polling in latency-critical code paths,
/// such as checking for RDMA completions or hardware events where blocking
/// or sleeping would add unwanted latency.
///
/// # Arguments
/// * `event` - A closure that returns `Ok(value)` when the desired condition is met,
///   or `Err(error)` to indicate that the condition is not yet satisfied.
/// * `timeout` - The maximum duration to spin before returning the **last error**.
///
/// # Returns
/// * `Ok((value, duration))` — If `event` returns `Ok(value)` before the timeout expires.
///   The duration is the time elapsed since the call to this function.
/// * `Err(error)` — If the timeout duration elapses before a successful result is returned.
///   The `error` is the last error observed from `event`.
///
/// # Notes
/// This function may be CPU-intensive as it does not yield or sleep between iterations.
/// Use only in contexts where minimal latency is critical.
pub fn spin_poll_timeout<T, E>(
    mut event: impl FnMut() -> Result<T, E>,
    timeout: Duration,
) -> Result<(T, Duration), E> {
    let start_time = Instant::now();
    let mut last_error;

    loop {
        match event() {
            Ok(success) => return Ok((success, start_time.elapsed())),
            Err(error) => last_error = Some(error),
        }

        if start_time.elapsed() > timeout {
            // Unwrap is safe: we've encountered at least one Err before timeout.
            return Err(last_error.unwrap());
        }
    }
}

/// Similar to [`spin_poll_timeout`], but checks the timeout only once every
/// `batch_iters` iterations to reduce timing overhead.
///
/// This can be more efficient when the `event` closure is expected to complete
/// quickly and the cost of calling [`Instant::elapsed`] on every iteration
/// is non-negligible.
///
/// # Arguments
/// * `event` - A closure that returns `Ok(value)` when the desired condition is met,
///   or `Err(error)` to indicate that the condition is not yet satisfied.
/// * `timeout` - The maximum duration to spin before returning the **last error**.
/// * `batch_iters` - The number of polling iterations to perform before rechecking
///   the timeout.
///
/// # Returns
/// * `Ok((value, duration))` — If `event` returns `Ok(value)` before the timeout expires.
///   The duration is the time elapsed since the call to this function.
/// * `Err(error)` — If the timeout duration elapses before a successful result is returned.
///   The `error` is the last error observed from `event`.
///
/// # Notes
/// This function performs a busy-wait loop without sleeping or yielding.
/// It should be used only in latency-critical paths where minimal delay is required.
pub fn spin_poll_timeout_batched<T, E>(
    mut event: impl FnMut() -> Result<T, E>,
    timeout: Duration,
    batch_iters: usize,
) -> Result<(T, Duration), E> {
    let start_time = Instant::now();
    let mut batch_counter = 0;
    let mut last_error;

    loop {
        match event() {
            Ok(success) => return Ok((success, start_time.elapsed())),
            Err(error) => last_error = Some(error),
        }

        if batch_counter >= batch_iters {
            if start_time.elapsed() > timeout {
                // Unwrap is safe: we've encountered at least one Err before timeout.
                return Err(last_error.unwrap());
            }
            batch_counter = 0;
        } else {
            batch_counter += 1;
        }
    }
}

/// Spins in a tight loop, repeatedly calling `event` until it returns `Ok(_)`
/// or the given number of retries is exhausted.
///
/// This is useful when you want deterministic retry behavior (a fixed number
/// of attempts) rather than a time-based timeout.
///
/// # Arguments
/// * `event` - A closure that returns `Ok(value)` when the desired condition is met,
///   or `Err(error)` to indicate that the condition is not yet satisfied.
/// * `retries` - The maximum number of times to call `event` before returning
///   the **last error**.
///
/// # Returns
/// * `Ok((value, duration))` — If `event` returns `Ok(value)` before the retry
///   limit is reached. The duration is the time elapsed since the start of polling.
/// * `Err(error)` — If all retries are exhausted without success. The `error`
///   is the last error observed from `event`.
///
/// # Notes
/// This function performs a busy-wait loop without sleeping or yielding.
/// It should be used only in latency-critical paths where minimal delay is required.
pub fn spin_poll_retries<T, E>(
    mut event: impl FnMut() -> Result<T, E>,
    retries: usize,
) -> Result<(T, std::time::Duration), E> {
    let start_time = Instant::now();
    let mut last_error = None;

    for _ in 0..retries {
        match event() {
            Ok(success) => return Ok((success, start_time.elapsed())),
            Err(error) => last_error = Some(error),
        }
    }

    // Unwrap is safe since we ran at least one iteration
    Err(last_error.unwrap())
}

/// Spins in a tight loop, repeatedly calling `event` until it returns `Ok(_)`,
/// the timeout expires, or the maximum number of retries is reached.
///
/// # Arguments
/// * `event` - A closure that returns `Ok(value)` when the desired condition is met,
///   or `Err(error)` to indicate that the condition is not yet satisfied.
/// * `timeout` - The maximum duration to spin before returning the **last error**.
/// * `max_retries` - The maximum number of times to call `event` before giving up.
///
/// # Returns
/// * `Ok((value, duration))` — If `event` returns `Ok(value)` before either
///   limit is reached. The duration is the time elapsed since the call to this function.
/// * `Err(error)` — If the timeout expires or the retry limit is exceeded without success.
///   The `error` is the last error observed from `event`.
///
/// # Notes
/// This function performs a busy-wait loop without sleeping or yielding.
/// It should be used only in latency-critical paths where minimal delay is required.
pub fn spin_poll_timeout_retries<T, E>(
    mut event: impl FnMut() -> Result<T, E>,
    timeout: Duration,
    max_retries: usize,
) -> Result<(T, Duration), E> {
    let start_time = Instant::now();
    let mut last_error = None;

    for _ in 0..max_retries {
        match event() {
            Ok(success) => return Ok((success, start_time.elapsed())),
            Err(error) => last_error = Some(error),
        }

        if start_time.elapsed() > timeout {
            // Timeout exceeded
            return Err(last_error.unwrap());
        }
    }

    // Retry limit reached
    Err(last_error.unwrap())
}

/// Similar to [`spin_poll_timeout_retries`], but checks the timeout only once every
/// `batch_iters` iterations to reduce timing overhead.
///
/// This variant terminates as soon as **either** the timeout expires or the
/// maximum number of retries is reached, whichever comes first.
///
/// This function is ideal for extremely fast polling loops where the cost of
/// calling [`Instant::elapsed`] on every iteration is non-negligible, and both
/// a retry limit and timeout are desired safeguards.
///
/// # Arguments
/// * `event` - A closure that returns `Ok(value)` when the desired condition is met,
///   or `Err(error)` to indicate that the condition is not yet satisfied.
/// * `timeout` - The maximum duration to spin before returning the **last error**.
/// * `max_retries` - The maximum number of times to call `event` before giving up.
/// * `batch_iters` - The number of polling iterations to perform before rechecking
///   the timeout.
///
/// # Returns
/// * `Ok((value, duration))` — If `event` returns `Ok(value)` before either
///   limit is reached. The duration is the time elapsed since the call to this function.
/// * `Err(error)` — If the timeout expires or the retry limit is exceeded without success.
///   The `error` is the last error observed from `event`.
///
/// # Notes
/// This function performs a busy-wait loop without sleeping or yielding.
/// It should be used only in latency-critical paths where minimal delay is required.
pub fn spin_poll_timeout_retries_batched<T, E>(
    mut event: impl FnMut() -> Result<T, E>,
    timeout: Duration,
    max_retries: usize,
    batch_iters: usize,
) -> Result<(T, Duration), E> {
    let start_time = Instant::now();
    let mut batch_counter = 0;
    let mut last_error = None;

    for _ in 0..max_retries {
        match event() {
            Ok(success) => return Ok((success, start_time.elapsed())),
            Err(error) => last_error = Some(error),
        }

        if batch_counter >= batch_iters {
            if start_time.elapsed() > timeout {
                // Timeout exceeded
                return Err(last_error.unwrap());
            }
            batch_counter = 0;
        } else {
            batch_counter += 1;
        }
    }

    // Retry limit reached
    Err(last_error.unwrap())
}
