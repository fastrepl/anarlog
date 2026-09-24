#if ANARLOG_ZOOM_SDK
  import Foundation
  import ZoomBridgeProtocol
  import ZoomSDKShim

  final class ZoomSdkMeetingDriver: NSObject, MeetingDriver, ANLGZoomSessionDelegate {
    var onEvent: ((DriverEvent) -> Void)?

    private let session = ANLGZoomSession()
    private var captureStartedAt: Date?
    private var admitted = false
    private var leaving = false

    override init() {
      super.init()
      session.delegate = self
    }

    func join(
      meetingNumber: Int64, passcode: String?, displayName: String,
      credentials: MeetingSdkJwt.Credentials?
    ) {
      guard let credentials else {
        onEvent?(
          .terminal(
            .failed,
            TerminalReason(
              kind: .authenticationFailed,
              message: "ANARLOG_ZOOM_SDK_KEY / ANARLOG_ZOOM_SDK_SECRET are not set")))
        return
      }
      let jwt = MeetingSdkJwt.mint(credentials, meetingNumber: meetingNumber)
      let code = session.start(
        withJWT: jwt, meetingNumber: meetingNumber, passcode: passcode, displayName: displayName)
      if code != 0 {
        onEvent?(
          .terminal(
            .failed,
            TerminalReason(kind: .providerError, message: "zoom sdk init/auth returned \(code)")))
      }
    }

    func leave() {
      leaving = true
      session.leave()
    }

    func shutdown() {
      session.shutdown()
    }

    // MARK: ANLGZoomSessionDelegate

    func zoomAuthFailed(withCode code: Int) {
      onEvent?(
        .terminal(
          .failed,
          TerminalReason(kind: .authenticationFailed, message: "zoom sdk auth error \(code)")))
    }

    func zoomStatusChanged(_ status: ANLGZoomStatus, code: Int) {
      switch status {
      case .connecting:
        break
      case .waitingForHost, .inWaitingRoom:
        onEvent?(.payload(.waitingForAdmission))
      case .inMeeting:
        guard !admitted else { return }
        admitted = true
        onEvent?(.payload(.joined))
        captureStartedAt = Date()
        onEvent?(.payload(.capturing))
      case .disconnecting:
        break
      case .ended:
        // `Canceled` is only reachable before the meeting was joined; afterwards a
        // user-requested leave still completes the capture.
        onEvent?(
          .terminal(
            admitted ? .completed : .canceled,
            TerminalReason(kind: leaving ? .stoppedByRequest : .meetingEnded)))
      case .failed:
        onEvent?(
          .terminal(
            .failed,
            TerminalReason(kind: .providerError, message: "zoom meeting error \(code)")))
      @unknown default:
        break
      }
    }

    func zoomParticipantsChanged(_ participants: [[String: Any]]) {
      guard admitted else { return }
      for entry in participants {
        guard let id = entry["id"] as? String, !id.isEmpty else { continue }
        let name = (entry["name"] as? String).flatMap { $0.isEmpty ? nil : $0 }
        onEvent?(.payload(.participantUpserted(Participant(id: id, displayName: name))))
      }
    }

    func zoomParticipantsLeft(_ participantIDs: [String]) {
      guard admitted else { return }
      for id in participantIDs {
        onEvent?(.payload(.participantLeft(participantId: id)))
      }
    }

    func zoomActiveSpeakersChanged(_ participantIDs: [String]) {
      guard let startedAt = captureStartedAt else { return }
      let atMs = UInt64(max(0, Date().timeIntervalSince(startedAt) * 1000))
      onEvent?(
        .payload(.activeSpeakers(ActiveSpeakers(atMs: atMs, participantIds: participantIDs))))
    }
  }
#endif
