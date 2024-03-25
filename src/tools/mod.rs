use self::error::FluteError;
use self::error::Result;
use std::time::SystemTime;

/// Handle errors
pub mod error;
pub mod ringbuffer;

/// Convert the `SystemTime`into NTP.
pub fn system_time_to_ntp(time: SystemTime) -> Result<u64> {
    let duration = time
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| FluteError::new("Fail to get UNIX time"))?;
    let seconds_utc = duration.as_secs();
    let submicro = duration.subsec_micros();

    let seconds_ntp = seconds_utc + 2208988800u64;
    let fraction = (((submicro as u128) * (1u128 << 32)) / 1000000u128) as u32;
    Ok((seconds_ntp << 32) | (fraction as u64))
}

/// Convert NTP to SystemTime
pub fn ntp_to_system_time(ntp: u64) -> Result<SystemTime> {
    let seconds_ntp = ntp >> 32;
    if seconds_ntp < 2208988800u64 {
        return Err(FluteError::new(format!(
            "Invalid NTP seconds {}",
            seconds_ntp
        )));
    }
    let seconds_utc = seconds_ntp - 2208988800u64;
    let fraction = ntp & 0xFFFFFFFF;
    let submicro = ((fraction as u128 * 1000000u128) / (1u128 << 32)) as u64;
    let utc_micro = (seconds_utc * 1000000u64) + submicro;
    Ok(SystemTime::UNIX_EPOCH + std::time::Duration::from_micros(utc_micro))
}
