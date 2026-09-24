import Foundation
import XCTest

@testable import ZoomBridgeProtocol

final class BridgeProtocolTests: XCTestCase {
  // Byte-for-byte what `serde_json::to_vec(&MeetingSdkBridgeCommand::Start(..))` emits
  // from `plugins/zoom-client/src/ext.rs`.
  let startLine = """
    {"type":"start","data":{"protocolVersion":1,"checkpoint":{"job_id":"zoom-1","bot_id":"zoom-self-1",\
    "provider":"zoom_meeting_sdk","meeting":{"platform":"zoom","url":"https://us02web.zoom.us/j/1234567890?pwd=abc"},\
    "state":"queued","next_sequence":0},"botName":"John"}}
    """

  func testDecodesRustStartCommand() throws {
    let command = try JSONDecoder().decode(BridgeCommand.self, from: Data(startLine.utf8))
    guard case .start(let start) = command else { return XCTFail("expected start") }
    XCTAssertEqual(start.protocolVersion, 1)
    XCTAssertEqual(start.checkpoint.provider, .zoomMeetingSdk)
    XCTAssertEqual(start.checkpoint.meeting.platform, .zoom)
    XCTAssertEqual(start.checkpoint.state, .queued)
    XCTAssertEqual(start.botName, "John")
  }

  func testDecodesStopCommand() throws {
    let command = try JSONDecoder().decode(
      BridgeCommand.self, from: Data(#"{"type":"stop","data":{"job_id":"zoom-1"}}"#.utf8))
    XCTAssertEqual(command, .stop(jobId: "zoom-1"))
  }

  func testEventStreamMatchesRustWireFormat() throws {
    let command = try JSONDecoder().decode(BridgeCommand.self, from: Data(startLine.utf8))
    guard case .start(let start) = command else { return XCTFail("expected start") }
    var lines: [String] = []
    let stream = BridgeEventStream(checkpoint: start.checkpoint) {
      lines.append(String(decoding: $0, as: UTF8.self))
    }
    stream.emit(.ready)
    stream.emit(.activeSpeakers(ActiveSpeakers(atMs: 1500, participantIds: ["7"])))
    stream.emit(.participantLeft(participantId: "7"))
    stream.emit(
      .terminal(BridgeTerminal(state: .completed, reason: TerminalReason(kind: .meetingEnded))))

    XCTAssertEqual(
      lines[0],
      #"{"payload":{"type":"ready"},"platform":"zoom","protocolVersion":1,"provider":"zoom_meeting_sdk","sequence":0}"#
        + "\n")
    XCTAssertEqual(
      lines[1],
      #"{"payload":{"data":{"at_ms":1500,"participant_ids":["7"]},"type":"active_speakers"},"platform":"zoom","protocolVersion":1,"provider":"zoom_meeting_sdk","sequence":1}"#
        + "\n")
    XCTAssertEqual(
      lines[2],
      #"{"payload":{"data":{"participant_id":"7"},"type":"participant_left"},"platform":"zoom","protocolVersion":1,"provider":"zoom_meeting_sdk","sequence":2}"#
        + "\n")
    XCTAssertEqual(
      lines[3],
      #"{"payload":{"data":{"reason":{"kind":"meeting_ended","retryable":false},"state":"completed"},"type":"terminal"},"platform":"zoom","protocolVersion":1,"provider":"zoom_meeting_sdk","sequence":3}"#
        + "\n")
  }

  func testParsesZoomJoinLinks() {
    XCTAssertEqual(
      ZoomMeetingURL.parse("https://us02web.zoom.us/j/1234567890?pwd=abc.def")?.meetingNumber,
      1_234_567_890)
    XCTAssertEqual(
      ZoomMeetingURL.parse("https://us02web.zoom.us/j/1234567890?pwd=abc.def")?.passcode, "abc.def")
    XCTAssertNil(ZoomMeetingURL.parse("https://zoom.us/j/1234567890")?.passcode)
    XCTAssertEqual(ZoomMeetingURL.parse("https://zoom.us/w/987654321")?.meetingNumber, 987_654_321)
    XCTAssertNil(ZoomMeetingURL.parse("https://zoom.us/signin"))
    XCTAssertNil(ZoomMeetingURL.parse("not a url"))
  }

  func testMintsHs256Jwt() {
    let token = MeetingSdkJwt.mint(
      .init(key: "key", secret: "secret"), meetingNumber: 42,
      now: Date(timeIntervalSince1970: 1_000_000))
    let parts = token.split(separator: ".")
    XCTAssertEqual(parts.count, 3)
    XCTAssertEqual(String(parts[0]), "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9")
    let payload = Data(base64Encoded: String(parts[1]).padding(toLength: 4 * ((parts[1].count + 3) / 4), withPad: "=", startingAt: 0).replacingOccurrences(of: "-", with: "+").replacingOccurrences(of: "_", with: "/"))!
    let json = try! JSONSerialization.jsonObject(with: payload) as! [String: Any]
    XCTAssertEqual(json["sdkKey"] as? String, "key")
    XCTAssertEqual(json["mn"] as? String, "42")
    XCTAssertEqual(json["iat"] as? Int, 999_970)
    XCTAssertEqual(json["exp"] as? Int, 999_970 + 7200)
    XCTAssertFalse(token.contains("secret"))
  }
}
