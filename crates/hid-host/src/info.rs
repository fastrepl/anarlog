use std::borrow::Cow;
use std::ffi::CString;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HidDeviceInfo {
    pub path: CString,
    pub vendor_id: u16,
    pub product_id: u16,
    pub serial_number: Option<String>,
    pub release_number: u16,
    pub manufacturer_string: Option<String>,
    pub product_string: Option<String>,
    pub usage_page: u16,
    pub usage: u16,
    pub interface_number: i32,
}

impl HidDeviceInfo {
    pub fn path_string_lossy(&self) -> Cow<'_, str> {
        self.path.to_string_lossy()
    }
}

impl From<&hidapi::DeviceInfo> for HidDeviceInfo {
    fn from(value: &hidapi::DeviceInfo) -> Self {
        Self {
            path: value.path().to_owned(),
            vendor_id: value.vendor_id(),
            product_id: value.product_id(),
            serial_number: value.serial_number().map(ToOwned::to_owned),
            release_number: value.release_number(),
            manufacturer_string: value.manufacturer_string().map(ToOwned::to_owned),
            product_string: value.product_string().map(ToOwned::to_owned),
            usage_page: value.usage_page(),
            usage: value.usage(),
            interface_number: value.interface_number(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HidDeviceFilter {
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
    pub usage_page: Option<u16>,
    pub usage: Option<u16>,
    pub serial_number: Option<String>,
    pub manufacturer_string: Option<String>,
    pub product_string: Option<String>,
    pub interface_number: Option<i32>,
}

impl HidDeviceFilter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn vendor_product(vendor_id: u16, product_id: u16) -> Self {
        Self::new()
            .with_vendor_id(vendor_id)
            .with_product_id(product_id)
    }

    pub fn with_vendor_id(mut self, vendor_id: u16) -> Self {
        self.vendor_id = Some(vendor_id);
        self
    }

    pub fn with_product_id(mut self, product_id: u16) -> Self {
        self.product_id = Some(product_id);
        self
    }

    pub fn with_usage_page(mut self, usage_page: u16) -> Self {
        self.usage_page = Some(usage_page);
        self
    }

    pub fn with_usage(mut self, usage: u16) -> Self {
        self.usage = Some(usage);
        self
    }

    pub fn with_serial_number(mut self, serial_number: impl Into<String>) -> Self {
        self.serial_number = Some(serial_number.into());
        self
    }

    pub fn with_manufacturer_string(mut self, manufacturer_string: impl Into<String>) -> Self {
        self.manufacturer_string = Some(manufacturer_string.into());
        self
    }

    pub fn with_product_string(mut self, product_string: impl Into<String>) -> Self {
        self.product_string = Some(product_string.into());
        self
    }

    pub fn with_interface_number(mut self, interface_number: i32) -> Self {
        self.interface_number = Some(interface_number);
        self
    }

    pub fn matches(&self, info: &HidDeviceInfo) -> bool {
        self.vendor_id
            .is_none_or(|expected| expected == info.vendor_id)
            && self
                .product_id
                .is_none_or(|expected| expected == info.product_id)
            && self
                .usage_page
                .is_none_or(|expected| expected == info.usage_page)
            && self.usage.is_none_or(|expected| expected == info.usage)
            && self
                .serial_number
                .as_deref()
                .is_none_or(|expected| info.serial_number.as_deref() == Some(expected))
            && self
                .manufacturer_string
                .as_deref()
                .is_none_or(|expected| info.manufacturer_string.as_deref() == Some(expected))
            && self
                .product_string
                .as_deref()
                .is_none_or(|expected| info.product_string.as_deref() == Some(expected))
            && self
                .interface_number
                .is_none_or(|expected| expected == info.interface_number)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_info() -> HidDeviceInfo {
        HidDeviceInfo {
            path: CString::new("/dev/hidraw0").unwrap(),
            vendor_id: 0x1234,
            product_id: 0x5678,
            serial_number: Some("abc123".to_string()),
            release_number: 0x0100,
            manufacturer_string: Some("Hypr".to_string()),
            product_string: Some("Dongle".to_string()),
            usage_page: 0xff00,
            usage: 0x0001,
            interface_number: 2,
        }
    }

    #[test]
    fn filter_matches_expected_device() {
        let filter = HidDeviceFilter::vendor_product(0x1234, 0x5678)
            .with_usage_page(0xff00)
            .with_usage(0x0001)
            .with_product_string("Dongle")
            .with_serial_number("abc123")
            .with_interface_number(2);

        assert!(filter.matches(&sample_info()));
    }

    #[test]
    fn filter_rejects_non_matching_device() {
        let filter = HidDeviceFilter::vendor_product(0x1234, 0x9999);

        assert!(!filter.matches(&sample_info()));
    }
}
