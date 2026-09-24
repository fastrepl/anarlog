import Foundation
import ZoomBridgeProtocol

enum DriverEvent {
  case payload(BridgeEventPayload)
  case terminal(BotState, TerminalReason)
}

protocol MeetingDriver: AnyObject {
  var onEvent: ((DriverEvent) -> Void)? { get set }
  func join(
    meetingNumber: Int64, passcode: String?, displayName: String,
    credentials: MeetingSdkJwt.Credentials?)
  func leave()
  func shutdown()
}

/// Built when the proprietary Zoom SDK is absent: keeps the bridge contract exercisable
/// end-to-end (ready → terminal) so the desktop plugin can be developed on any machine.
final class StubMeetingDriver: MeetingDriver {
  var onEvent: ((DriverEvent) -> Void)?

  func join(
    meetingNumber: Int64, passcode: String?, displayName: String,
    credentials: MeetingSdkJwt.Credentials?
  ) {
    onEvent?(
      .terminal(
        .failed,
        TerminalReason(
          kind: .providerError,
          message: "char-sidecar-zoom was built without the Zoom Meeting SDK (set ANARLOG_ZOOM_SDK_PATH)"
        )))
  }

  func leave() {}
  func shutdown() {}
}

func makeMeetingDriver() -> MeetingDriver {
  #if ANARLOG_ZOOM_SDK
    return ZoomSdkMeetingDriver()
  #else
    return StubMeetingDriver()
  #endif
}
