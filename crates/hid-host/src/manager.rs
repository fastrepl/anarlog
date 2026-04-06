use std::ffi::CStr;
use std::sync::Mutex;

use crate::error::{Error, Result};
use crate::info::{HidDeviceFilter, HidDeviceInfo};
use crate::transport::{HidConnection, HidReportConfig};

pub struct HidManager {
    api: Mutex<hidapi::HidApi>,
}

impl HidManager {
    pub fn new() -> Result<Self> {
        Ok(Self {
            api: Mutex::new(hidapi::HidApi::new()?),
        })
    }

    pub fn refresh(&self) -> Result<()> {
        let mut api = self.api.lock().unwrap();
        api.refresh_devices()?;
        Ok(())
    }

    pub fn devices(&self) -> Result<Vec<HidDeviceInfo>> {
        let mut api = self.api.lock().unwrap();
        api.refresh_devices()?;
        Ok(api.device_list().map(HidDeviceInfo::from).collect())
    }

    pub fn find(&self, filter: &HidDeviceFilter) -> Result<Vec<HidDeviceInfo>> {
        Ok(self
            .devices()?
            .into_iter()
            .filter(|info| filter.matches(info))
            .collect())
    }

    pub fn open_first(
        &self,
        filter: &HidDeviceFilter,
        report_config: HidReportConfig,
    ) -> Result<HidConnection> {
        let mut api = self.api.lock().unwrap();
        api.refresh_devices()?;

        for raw_info in api.device_list() {
            let info = HidDeviceInfo::from(raw_info);
            if !filter.matches(&info) {
                continue;
            }

            tracing::debug!(
                vendor_id = info.vendor_id,
                product_id = info.product_id,
                path = %info.path_string_lossy(),
                "hid_device_open_first"
            );

            let device = raw_info.open_device(&api)?;
            return Ok(HidConnection::new(device, info, report_config));
        }

        Err(Error::DeviceNotFound {
            filter: filter.clone(),
        })
    }

    pub fn open_unique(
        &self,
        filter: &HidDeviceFilter,
        report_config: HidReportConfig,
    ) -> Result<HidConnection> {
        let matches = self.find(filter)?;

        match matches.len() {
            0 => Err(Error::DeviceNotFound {
                filter: filter.clone(),
            }),
            1 => {
                let info = matches.into_iter().next().unwrap();
                tracing::debug!(
                    vendor_id = info.vendor_id,
                    product_id = info.product_id,
                    path = %info.path_string_lossy(),
                    "hid_device_open_unique"
                );
                let api = self.api.lock().unwrap();
                let device = api.open_path(&info.path)?;
                Ok(HidConnection::new(device, info, report_config))
            }
            count => Err(Error::MultipleDevicesMatched {
                filter: filter.clone(),
                count,
            }),
        }
    }

    pub fn open_path(&self, path: &CStr, report_config: HidReportConfig) -> Result<HidConnection> {
        let api = self.api.lock().unwrap();
        let device = api.open_path(path)?;
        let info = HidDeviceInfo::from(&device.get_device_info()?);
        Ok(HidConnection::new(device, info, report_config))
    }
}
