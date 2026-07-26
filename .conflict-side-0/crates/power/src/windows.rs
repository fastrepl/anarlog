use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};

use crate::{Error, PowerSource, Snapshot, ThermalState};

const AC_LINE_OFFLINE: u8 = 0;
const AC_LINE_ONLINE: u8 = 1;
const BATTERY_FLAG_CHARGING: u8 = 0x08;
const BATTERY_FLAG_NO_SYSTEM_BATTERY: u8 = 0x80;
const STATUS_FLAG_BATTERY_SAVER_OFF: u8 = 0;
const STATUS_FLAG_BATTERY_SAVER_ON: u8 = 1;

pub fn snapshot() -> Result<Snapshot, Error> {
    let mut status = SYSTEM_POWER_STATUS::default();
    unsafe { GetSystemPowerStatus(&mut status) }
        .map_err(|_| Error::Unavailable("GetSystemPowerStatus"))?;

    let has_battery = has_battery(status.BatteryFlag);

    Ok(Snapshot {
        has_battery,
        power_source: power_source(status.ACLineStatus),
        is_charging: has_battery.then(|| is_charging(status.BatteryFlag)),
        battery_percent: has_battery
            .then(|| status.BatteryLifePercent)
            .filter(|&v| v <= 100),
        low_power_mode: low_power_mode(status.SystemStatusFlag),
        thermal_state: ThermalState::Unknown,
    })
}

fn has_battery(battery_flag: u8) -> bool {
    battery_flag != BATTERY_FLAG_NO_SYSTEM_BATTERY
}

fn is_charging(battery_flag: u8) -> bool {
    battery_flag & BATTERY_FLAG_CHARGING != 0
}

fn power_source(ac_line_status: u8) -> PowerSource {
    match ac_line_status {
        AC_LINE_OFFLINE => PowerSource::Battery,
        AC_LINE_ONLINE => PowerSource::Ac,
        _ => PowerSource::Unknown,
    }
}

fn low_power_mode(system_status_flag: u8) -> bool {
    match system_status_flag {
        STATUS_FLAG_BATTERY_SAVER_OFF => false,
        STATUS_FLAG_BATTERY_SAVER_ON => true,
        _ => false,
    }
}
