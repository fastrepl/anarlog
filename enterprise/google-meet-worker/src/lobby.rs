// Adapted for Anarlog from Vexa v0.12.18. See ../THIRD_PARTY_NOTICES.md.

use std::time::Duration;

use serde::Deserialize;

use crate::{CdpError, CdpPage, X11Input, X11InputError};

const LOBBY_PROBE_EXPRESSION: &str = include_str!("lobby_probe.js");
const INSTALL_MOUSE_PROBE: &str = r#"(() => {
  if (window.__anlgMouseProbeInstalled) return true;
  window.__anlgMouseProbeInstalled = true;
  window.__anlgLastMouse = null;
  window.addEventListener("mousemove", (event) => {
    window.__anlgLastMouse = { x: event.clientX, y: event.clientY };
  }, { capture: true });
  return true;
})()"#;
const LAST_MOUSE_POSITION: &str = "window.__anlgLastMouse || null";
const TARGET_VERIFY_ATTEMPTS: usize = 3;
const LOBBY_READY_TIMEOUT: Duration = Duration::from_secs(30);
const LOBBY_READY_POLL_INTERVAL: Duration = Duration::from_millis(250);
const POINTER_SETTLE: Duration = Duration::from_millis(120);
const CLICK_HOLD: Duration = Duration::from_millis(70);
const NAME_TYPING_DELAY: Duration = Duration::from_millis(75);

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct PagePoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct LobbyTarget {
    pub center_x: f64,
    pub center_y: f64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct LobbySnapshot {
    pub screen_x: f64,
    pub screen_y: f64,
    pub inner_width: f64,
    pub inner_height: f64,
    pub device_pixel_ratio: f64,
    pub signed_out_lobby: bool,
    pub name_input: Option<LobbyTarget>,
    pub join_cta: Option<LobbyTarget>,
    pub microphone_on: Option<LobbyTarget>,
    pub camera_on: Option<LobbyTarget>,
    #[serde(default)]
    pub device_error_dismissal: Option<LobbyTarget>,
    #[serde(default)]
    pub cta_candidates: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
struct PointerCalibration {
    offset_x: f64,
    offset_y: f64,
    device_pixel_ratio: f64,
}

#[derive(Debug, Clone, Copy)]
enum LobbyTargetKind {
    NameInput,
    JoinCta,
    MicrophoneOn,
    CameraOn,
    DeviceErrorDismissal,
}

impl LobbyTargetKind {
    fn marker(self) -> &'static str {
        match self {
            Self::NameInput => "name_input",
            Self::JoinCta => "join_cta",
            Self::MicrophoneOn => "microphone_on",
            Self::CameraOn => "camera_on",
            Self::DeviceErrorDismissal => "device_error_dismissal",
        }
    }

    fn target_in(self, snapshot: &LobbySnapshot) -> Option<LobbyTarget> {
        match self {
            Self::NameInput => snapshot.name_input,
            Self::JoinCta => snapshot.join_cta,
            Self::MicrophoneOn => snapshot.microphone_on,
            Self::CameraOn => snapshot.camera_on,
            Self::DeviceErrorDismissal => snapshot.device_error_dismissal,
        }
    }
}

pub struct LobbyController<'a> {
    page: &'a mut CdpPage,
    input: &'a X11Input,
    calibration: Option<PointerCalibration>,
}

impl<'a> LobbyController<'a> {
    pub fn new(page: &'a mut CdpPage, input: &'a X11Input) -> Self {
        Self {
            page,
            input,
            calibration: None,
        }
    }

    pub async fn join(&mut self, bot_name: &str, authenticated: bool) -> Result<(), LobbyError> {
        self.input.verify_available().await?;
        self.wait_until_ready(authenticated).await?;
        if !authenticated {
            X11Input::validate_text(bot_name, NAME_TYPING_DELAY)?;
            self.trusted_click(LobbyTargetKind::NameInput).await?;
            self.input.type_text(bot_name, NAME_TYPING_DELAY).await?;
        }
        self.wait_until_joinable().await?;
        self.click_if_present(LobbyTargetKind::MicrophoneOn).await?;
        self.click_if_present(LobbyTargetKind::CameraOn).await?;
        self.trusted_click(LobbyTargetKind::JoinCta).await
    }

    async fn wait_until_ready(&mut self, authenticated: bool) -> Result<(), LobbyError> {
        let deadline = tokio::time::Instant::now() + LOBBY_READY_TIMEOUT;
        loop {
            let snapshot = self.probe().await?;
            if snapshot.device_error_dismissal.is_some() {
                self.click_if_present(LobbyTargetKind::DeviceErrorDismissal)
                    .await?;
                continue;
            }
            if authenticated && snapshot.signed_out_lobby && snapshot.name_input.is_some() {
                return Err(LobbyError::AuthenticatedProfileSignedOut);
            }
            let lobby_ready = if authenticated {
                snapshot.join_cta.is_some()
            } else {
                snapshot.name_input.is_some()
            };
            if lobby_ready {
                return Ok(());
            }
            let now = tokio::time::Instant::now();
            if now >= deadline {
                return Err(LobbyError::LobbyNotReady {
                    timeout: LOBBY_READY_TIMEOUT,
                    cta_candidates: snapshot.cta_candidates,
                });
            }
            tokio::time::sleep(LOBBY_READY_POLL_INTERVAL.min(deadline - now)).await;
        }
    }

    async fn wait_until_joinable(&mut self) -> Result<(), LobbyError> {
        let deadline = tokio::time::Instant::now() + LOBBY_READY_TIMEOUT;
        loop {
            let snapshot = self.probe().await?;
            if snapshot.device_error_dismissal.is_some() {
                self.click_if_present(LobbyTargetKind::DeviceErrorDismissal)
                    .await?;
                continue;
            }
            if snapshot.join_cta.is_some() {
                return Ok(());
            }
            let now = tokio::time::Instant::now();
            if now >= deadline {
                return Err(LobbyError::LobbyNotReady {
                    timeout: LOBBY_READY_TIMEOUT,
                    cta_candidates: snapshot.cta_candidates,
                });
            }
            tokio::time::sleep(LOBBY_READY_POLL_INTERVAL.min(deadline - now)).await;
        }
    }

    pub async fn probe(&mut self) -> Result<LobbySnapshot, LobbyError> {
        let snapshot: LobbySnapshot = self.page.evaluate(LOBBY_PROBE_EXPRESSION).await?;
        validate_geometry(&snapshot)?;
        Ok(snapshot)
    }

    async fn click_if_present(&mut self, kind: LobbyTargetKind) -> Result<(), LobbyError> {
        if kind.target_in(&self.probe().await?).is_some() {
            match self.trusted_click(kind).await {
                Err(LobbyError::TargetMissing(marker)) if marker == kind.marker() => {}
                result => result?,
            }
        }
        Ok(())
    }

    async fn trusted_click(&mut self, kind: LobbyTargetKind) -> Result<(), LobbyError> {
        for _ in 0..TARGET_VERIFY_ATTEMPTS {
            if self.calibration.is_none() {
                self.calibration = Some(self.calibrate().await?);
            }
            let calibration = self.calibration.expect("calibration was initialized");
            let snapshot = self.probe().await?;
            let target = kind
                .target_in(&snapshot)
                .ok_or(LobbyError::TargetMissing(kind.marker()))?;
            let target_x = to_x11_coordinate(
                calibration.offset_x + target.center_x * calibration.device_pixel_ratio,
            )?;
            let target_y = to_x11_coordinate(
                calibration.offset_y + target.center_y * calibration.device_pixel_ratio,
            )?;
            self.input.move_absolute(target_x, target_y).await?;
            tokio::time::sleep(POINTER_SETTLE).await;

            let pointer = self.input.pointer_location().await?;
            let page_x =
                (f64::from(pointer.x) - calibration.offset_x) / calibration.device_pixel_ratio;
            let page_y =
                (f64::from(pointer.y) - calibration.offset_y) / calibration.device_pixel_ratio;
            if self.verify_target(kind, page_x, page_y).await? {
                self.input.button_down().await?;
                tokio::time::sleep(CLICK_HOLD).await;
                self.input.button_up().await?;
                return Ok(());
            }
            self.calibration = None;
        }
        Err(LobbyError::TargetVerificationFailed(kind.marker()))
    }

    async fn calibrate(&mut self) -> Result<PointerCalibration, LobbyError> {
        let _: bool = self.page.evaluate(INSTALL_MOUSE_PROBE).await?;
        let geometry = self.probe().await?;
        let probes = [(0.35, 0.4), (0.6, 0.6)];
        for (x_ratio, y_ratio) in probes {
            let screen_x = to_x11_coordinate(
                (geometry.screen_x + geometry.inner_width * x_ratio) * geometry.device_pixel_ratio,
            )?;
            let screen_y = to_x11_coordinate(
                (geometry.screen_y + geometry.inner_height * y_ratio) * geometry.device_pixel_ratio,
            )?;
            self.input.move_absolute(screen_x, screen_y).await?;
            tokio::time::sleep(POINTER_SETTLE).await;
            let observed: Option<PagePoint> = self.page.evaluate(LAST_MOUSE_POSITION).await?;
            if let Some(observed) = observed {
                return Ok(PointerCalibration {
                    offset_x: f64::from(screen_x) - observed.x * geometry.device_pixel_ratio,
                    offset_y: f64::from(screen_y) - observed.y * geometry.device_pixel_ratio,
                    device_pixel_ratio: geometry.device_pixel_ratio,
                });
            }
        }
        Err(LobbyError::PointerCalibrationUnavailable)
    }

    async fn verify_target(
        &mut self,
        kind: LobbyTargetKind,
        page_x: f64,
        page_y: f64,
    ) -> Result<bool, LobbyError> {
        let expression = target_verification_expression(kind, page_x, page_y);
        self.page.evaluate(&expression).await.map_err(Into::into)
    }
}

fn target_verification_expression(kind: LobbyTargetKind, page_x: f64, page_y: f64) -> String {
    let marker = serde_json::to_string(kind.marker()).expect("static marker is serializable");
    format!(
        r#"(() => {{
  const marker = {marker};
  const target = document.querySelector(`[data-anlg-worker-target="${{marker}}"]`);
  const hit = document.elementFromPoint({page_x}, {page_y});
  return Boolean(target && hit && (target === hit || target.contains(hit) || hit.contains(target)));
}})()"#
    )
}

fn validate_geometry(snapshot: &LobbySnapshot) -> Result<(), LobbyError> {
    let values = [
        snapshot.screen_x,
        snapshot.screen_y,
        snapshot.inner_width,
        snapshot.inner_height,
        snapshot.device_pixel_ratio,
    ];
    if values.iter().any(|value| !value.is_finite())
        || snapshot.inner_width <= 0.0
        || snapshot.inner_height <= 0.0
        || snapshot.device_pixel_ratio <= 0.0
    {
        return Err(LobbyError::InvalidBrowserGeometry);
    }
    Ok(())
}

fn to_x11_coordinate(value: f64) -> Result<i32, LobbyError> {
    if !value.is_finite() || value < f64::from(i32::MIN) || value > f64::from(i32::MAX) {
        return Err(LobbyError::InvalidBrowserGeometry);
    }
    Ok(value.round() as i32)
}

#[derive(Debug, thiserror::Error)]
pub enum LobbyError {
    #[error(transparent)]
    Cdp(#[from] CdpError),
    #[error(transparent)]
    X11(#[from] X11InputError),
    #[error("authenticated Chromium profile is signed out")]
    AuthenticatedProfileSignedOut,
    #[error(
        "Google Meet lobby did not become ready within {timeout:?} (CTA candidates: {cta_candidates:?})"
    )]
    LobbyNotReady {
        timeout: Duration,
        cta_candidates: Vec<String>,
    },
    #[error("Google Meet lobby target is not available: {0}")]
    TargetMissing(&'static str),
    #[error("browser window geometry is invalid")]
    InvalidBrowserGeometry,
    #[error("real X11 pointer movement did not reach the browser page")]
    PointerCalibrationUnavailable,
    #[error("real X11 pointer could not be verified over lobby target: {0}")]
    TargetVerificationFailed(&'static str),
}

#[cfg(test)]
mod tests {
    use super::*;

    use futures_util::{SinkExt, StreamExt};
    use serde_json::{Value, json};
    use tokio::net::TcpListener;
    use tokio_tungstenite::{accept_async, connect_async, tungstenite::Message};

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    fn snapshot() -> LobbySnapshot {
        LobbySnapshot {
            screen_x: 0.0,
            screen_y: 0.0,
            inner_width: 1280.0,
            inner_height: 720.0,
            device_pixel_ratio: 2.0,
            signed_out_lobby: true,
            name_input: Some(LobbyTarget {
                center_x: 640.0,
                center_y: 300.0,
            }),
            join_cta: None,
            microphone_on: None,
            camera_on: None,
            device_error_dismissal: None,
            cta_candidates: vec![],
        }
    }

    #[test]
    fn rejects_non_finite_or_zero_browser_geometry() {
        for invalid in [f64::NAN, f64::INFINITY, 0.0, -1.0] {
            let mut value = snapshot();
            value.device_pixel_ratio = invalid;
            assert!(matches!(
                validate_geometry(&value),
                Err(LobbyError::InvalidBrowserGeometry)
            ));
        }
    }

    #[test]
    fn converts_page_targets_to_checked_x11_coordinates() {
        assert_eq!(to_x11_coordinate(123.6).unwrap(), 124);
        assert!(to_x11_coordinate(f64::NAN).is_err());
        assert!(to_x11_coordinate(f64::from(i32::MAX) + 1.0).is_err());
    }

    #[test]
    fn target_verification_uses_the_tagged_element_and_real_pointer_position() {
        let expression = target_verification_expression(LobbyTargetKind::JoinCta, 123.5, 456.25);

        assert!(expression.contains("const marker = \"join_cta\""));
        assert!(expression.contains("document.elementFromPoint(123.5, 456.25)"));
        assert!(expression.contains("data-anlg-worker-target"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn guest_join_uses_verified_xtest_input_for_name_and_cta() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut websocket = accept_async(stream).await.unwrap();
            let mut lobby_probes = 0;
            while let Some(Ok(Message::Text(command))) = websocket.next().await {
                let command: Value = serde_json::from_str(command.as_ref()).unwrap();
                let expression = command["params"]["expression"].as_str().unwrap();
                let value = if expression.contains("cta_candidates") {
                    lobby_probes += 1;
                    if lobby_probes == 1 {
                        json!({
                            "screen_x": 0.0,
                            "screen_y": 0.0,
                            "inner_width": 1000.0,
                            "inner_height": 720.0,
                            "device_pixel_ratio": 1.0,
                            "signed_out_lobby": false,
                            "name_input": null,
                            "join_cta": null,
                            "microphone_on": null,
                            "camera_on": null,
                            "cta_candidates": []
                        })
                    } else if lobby_probes <= 5 {
                        json!({
                            "screen_x": 0.0,
                            "screen_y": 0.0,
                            "inner_width": 1000.0,
                            "inner_height": 720.0,
                            "device_pixel_ratio": 1.0,
                            "signed_out_lobby": false,
                            "name_input": null,
                            "join_cta": null,
                            "microphone_on": null,
                            "camera_on": null,
                            "device_error_dismissal": {
                                "center_x": 500.0,
                                "center_y": 400.0
                            },
                            "cta_candidates": ["Close"]
                        })
                    } else if lobby_probes <= 7 {
                        json!({
                            "screen_x": 0.0,
                            "screen_y": 0.0,
                            "inner_width": 1000.0,
                            "inner_height": 720.0,
                            "device_pixel_ratio": 1.0,
                            "signed_out_lobby": true,
                            "name_input": {"center_x": 100.0, "center_y": 120.0},
                            "join_cta": null,
                            "microphone_on": null,
                            "camera_on": null,
                            "cta_candidates": []
                        })
                    } else {
                        json!({
                            "screen_x": 0.0,
                            "screen_y": 0.0,
                            "inner_width": 1000.0,
                            "inner_height": 720.0,
                            "device_pixel_ratio": 1.0,
                            "signed_out_lobby": true,
                            "name_input": {"center_x": 100.0, "center_y": 120.0},
                            "join_cta": {"center_x": 800.0, "center_y": 600.0},
                            "microphone_on": null,
                            "camera_on": null,
                            "cta_candidates": ["Ask to join"]
                        })
                    }
                } else if expression == LAST_MOUSE_POSITION {
                    json!({"x": 350.0, "y": 288.0})
                } else {
                    json!(true)
                };
                websocket
                    .send(Message::Text(
                        json!({
                            "id": command["id"],
                            "result": {"result": {"type": "object", "value": value}}
                        })
                        .to_string()
                        .into(),
                    ))
                    .await
                    .unwrap();
            }
        });
        let (websocket, _) = connect_async(format!("ws://{address}")).await.unwrap();
        let mut page = CdpPage::from_test_websocket(websocket);

        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join("xdotool");
        let pointer = directory.path().join("pointer");
        let calls = directory.path().join("calls");
        std::fs::write(&pointer, "0 0\n").unwrap();
        std::fs::write(
            &executable,
            format!(
                r#"#!/bin/sh
printf '%s\n' "$*" >> "{}"
if [ "$1" = "mousemove" ]; then
  printf '%s %s\n' "$2" "$3" > "{}"
elif [ "$1" = "getmouselocation" ]; then
  read x y < "{}"
  printf 'X=%s\nY=%s\nSCREEN=0\nWINDOW=1\n' "$x" "$y"
fi
"#,
                calls.display(),
                pointer.display(),
                pointer.display()
            ),
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&executable).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&executable, permissions).unwrap();
        let input = X11Input::new(crate::X11InputConfig {
            binary: executable,
            display: ":99".into(),
            command_timeout: Duration::from_secs(5),
        })
        .unwrap();

        LobbyController::new(&mut page, &input)
            .join("Anarlog Notes", false)
            .await
            .unwrap();
        page.close().await.unwrap();
        server.await.unwrap();

        let calls = std::fs::read_to_string(calls).unwrap();
        assert!(calls.contains("type --clearmodifiers --delay 75 -- Anarlog Notes"));
        assert_eq!(calls.matches("mousedown 1").count(), 3);
        assert_eq!(calls.matches("mouseup 1").count(), 3);
    }
}
