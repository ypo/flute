use self::error::FluteError;
use self::error::Result;
use std::time::SystemTime;

/// Handle errors
pub mod error;
pub mod ringbuffer;

/// Convert the `SystemTime`into NTP.
pub fn system_time_to_ntp(time: &SystemTime) -> Result<(u32, u32)> {
    let duration = time
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| FluteError::new("Fail to get UNIX time"))?;
    let seconds_utc = duration.as_secs();
    let seconds_utc_us = duration.as_micros();
    let seconds_ntp = seconds_utc as u32 + 2208988800u32;
    let rest_ntp = (((seconds_utc_us - (seconds_utc as u128 * 1000000u128)) * (1u128 << 32))
        / 1000000u128) as u32;
    Ok((seconds_ntp, rest_ntp))
}
