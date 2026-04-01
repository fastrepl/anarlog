use std::time::Duration;

use hypr_hid_interface::{DEFAULT_REPORT_ID, DEFAULT_REPORT_LEN, Packet, PacketType};

use crate::error::{Error, Result};
use crate::info::HidDeviceInfo;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HidReportConfig {
    pub report_id: u8,
    pub uses_report_ids: bool,
    pub input_report_len: usize,
    pub output_report_len: usize,
    pub feature_report_len: Option<usize>,
}

impl Default for HidReportConfig {
    fn default() -> Self {
        Self {
            report_id: DEFAULT_REPORT_ID,
            uses_report_ids: false,
            input_report_len: DEFAULT_REPORT_LEN,
            output_report_len: DEFAULT_REPORT_LEN,
            feature_report_len: Some(DEFAULT_REPORT_LEN),
        }
    }
}

#[derive(Debug)]
pub struct HidConnection {
    device: hidapi::HidDevice,
    info: HidDeviceInfo,
    config: HidReportConfig,
}

impl HidConnection {
    pub(crate) fn new(
        device: hidapi::HidDevice,
        info: HidDeviceInfo,
        config: HidReportConfig,
    ) -> Self {
        Self {
            device,
            info,
            config,
        }
    }

    pub fn info(&self) -> &HidDeviceInfo {
        &self.info
    }

    pub fn config(&self) -> HidReportConfig {
        self.config
    }

    pub fn report_descriptor(&self) -> Result<Vec<u8>> {
        let mut buf = vec![0; hidapi::MAX_REPORT_DESCRIPTOR_SIZE];
        let len = self.device.get_report_descriptor(&mut buf)?;
        buf.truncate(len);
        Ok(buf)
    }

    pub fn write_output(&self, payload: &[u8]) -> Result<usize> {
        let report = self.prepare_report(payload, self.config.output_report_len)?;
        Ok(self.device.write(&report)?)
    }

    pub fn send_output_report(&self, payload: &[u8]) -> Result<()> {
        let report = self.prepare_report(payload, self.config.output_report_len)?;
        self.device.send_output_report(&report)?;
        Ok(())
    }

    pub fn read_input(&self, timeout: Option<Duration>) -> Result<Vec<u8>> {
        let mut buf = vec![0; self.input_buffer_len()];
        let len = self
            .device
            .read_timeout(&mut buf, timeout_to_millis(timeout)?)?;
        if len == 0 {
            return Err(Error::ReadTimedOut);
        }

        if self.config.uses_report_ids {
            let report_id = buf[0];
            if report_id != self.config.report_id {
                return Err(Error::UnexpectedReportId {
                    expected: self.config.report_id,
                    actual: report_id,
                });
            }

            return Ok(buf[1..len].to_vec());
        }

        Ok(buf[..len].to_vec())
    }

    pub fn send_feature_report(&self, payload: &[u8]) -> Result<()> {
        let report_len = self
            .config
            .feature_report_len
            .ok_or(Error::FeatureReportsNotConfigured)?;
        let report = self.prepare_report(payload, report_len)?;
        self.device.send_feature_report(&report)?;
        Ok(())
    }

    pub fn get_feature_report(&self) -> Result<Vec<u8>> {
        let report_len = self
            .config
            .feature_report_len
            .ok_or(Error::FeatureReportsNotConfigured)?;
        let mut buf = vec![0; report_len + 1];
        buf[0] = self.config.report_id;
        let len = self.device.get_feature_report(&mut buf)?;
        if len == 0 {
            return Err(Error::ReadTimedOut);
        }
        Ok(buf[1..len].to_vec())
    }

    pub fn write_packet(&self, packet: &Packet) -> Result<usize> {
        self.write_output(&packet.encode()?).map_err(Into::into)
    }

    pub fn read_packet(&self, timeout: Option<Duration>) -> Result<Packet> {
        let report = self.read_input(timeout)?;
        Packet::decode(&report).map_err(Into::into)
    }

    pub fn exchange(&self, packet: &Packet, timeout: Option<Duration>) -> Result<Packet> {
        self.write_packet(packet)?;
        self.read_packet(timeout)
    }

    pub fn exchange_command(&self, packet: &Packet, timeout: Option<Duration>) -> Result<Packet> {
        let reply = self.exchange(packet, timeout)?;
        if reply.packet_type == PacketType::Response {
            return Ok(reply);
        }

        Err(Error::UnexpectedPacketType {
            expected: PacketType::Response,
            actual: reply.packet_type,
        })
    }

    fn prepare_report(&self, payload: &[u8], report_len: usize) -> Result<Vec<u8>> {
        if payload.len() > report_len {
            return Err(Error::ReportPayloadTooLarge {
                actual: payload.len(),
                max: report_len,
            });
        }

        let mut report = vec![0; report_len + 1];
        report[0] = self.config.report_id;
        report[1..1 + payload.len()].copy_from_slice(payload);
        Ok(report)
    }

    fn input_buffer_len(&self) -> usize {
        self.config.input_report_len + usize::from(self.config.uses_report_ids)
    }
}

fn timeout_to_millis(timeout: Option<Duration>) -> Result<i32> {
    match timeout {
        None => Ok(-1),
        Some(timeout) => {
            let millis = timeout.as_millis();
            if millis > i32::MAX as u128 {
                return Err(Error::TimeoutOverflow { millis });
            }

            Ok(millis as i32)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_report_config_matches_single_report_hid() {
        let config = HidReportConfig::default();

        assert_eq!(config.report_id, DEFAULT_REPORT_ID);
        assert!(!config.uses_report_ids);
        assert_eq!(config.input_report_len, DEFAULT_REPORT_LEN);
        assert_eq!(config.output_report_len, DEFAULT_REPORT_LEN);
        assert_eq!(config.feature_report_len, Some(DEFAULT_REPORT_LEN));
    }

    #[test]
    fn timeout_conversion_rejects_large_values() {
        let err = timeout_to_millis(Some(Duration::from_millis(i32::MAX as u64 + 1))).unwrap_err();

        assert!(matches!(err, Error::TimeoutOverflow { .. }));
    }
}
