import CaptureCore
import MeetingAccessibility
import XCTest

final class AdapterTests: XCTestCase {
  // Structure observed in Zoom 7.1.9 on Nov 2; names sanitized, no meeting links.
  func meeting(_ extra: [MeetingAXNode] = []) -> MeetingAXSnapshot {
    .init(
      nodes: [
        .init(id: "window", role: "AXWindow", identifier: "zm.meeting.window.main"),
        .init(id: "audio", parentID: "window", role: "AXButton", identifier: "audio"),
        .init(id: "leave", parentID: "window", role: "AXButton", identifier: "leave"),
        .init(id: "participants", parentID: "window", role: "AXButton", identifier: "participant"),
      ] + extra, complete: true)
  }

  func testRequiresInCallControlsInOneWindowNotPreviewOrCalendar() {
    let preview = MeetingAXSnapshot(
      nodes: [
        .init(id: "window", role: "AXWindow", identifier: "zm.meeting.window.main"),
        .init(id: "audio", parentID: "window", role: "AXButton", identifier: "Audio"),
        .init(id: "start", parentID: "window", role: "AXButton", title: "Start"),
      ], complete: true)
    XCTAssertEqual(
      MeetingAXAdapter.project(bundleID: "us.zoom.xos", snapshot: preview),
      .unknown("no_confirmed_meeting"))
    let other = meeting([
      .init(id: "other", role: "AXWindow", identifier: "zm.meeting.window.main"),
      .init(id: "audio2", parentID: "other", role: "AXButton", identifier: "audio"),
      .init(id: "leave2", parentID: "other", role: "AXButton", identifier: "leave"),
      .init(id: "participants2", parentID: "other", role: "AXButton", identifier: "participant"),
    ])
    XCTAssertEqual(
      MeetingAXAdapter.project(bundleID: "us.zoom.xos", snapshot: other),
      .unknown("multiple_meetings"))
    XCTAssertEqual(
      MeetingAXAdapter.project(bundleID: "other.app", snapshot: meeting()),
      .unknown("unsupported_application"))
  }

  func testProjectsOnlyRosterRowsAndNeverTreatsUnmutedAsSpeaking() {
    let snapshot = meeting([
      .init(id: "roster", parentID: "window", role: "AXOutline", description: "Participants list"),
      .init(id: "self", parentID: "roster", role: "AXRow"),
      .init(id: "self-name", parentID: "self", role: "AXStaticText", title: "Alex (Host, me)"),
      .init(id: "guest", parentID: "roster", role: "AXRow"),
      .init(id: "guest-name", parentID: "guest", role: "AXStaticText", title: "Alex"),
      .init(id: "mute", parentID: "guest", role: "AXButton", title: "Mute"),
      .init(id: "chat-name", parentID: "window", role: "AXStaticText", title: "Chat sender"),
    ])
    guard
      case .meeting(let result) = MeetingAXAdapter.project(
        bundleID: "us.zoom.xos", snapshot: snapshot)
    else {
      return XCTFail("Expected an observed meeting")
    }
    XCTAssertEqual(
      result.participants,
      [
        .init(id: "window/self-name", displayName: "Alex", isLocal: true),
        .init(id: "window/guest-name", displayName: "Alex", isLocal: false),
      ])
    XCTAssertEqual(result.speakers, [])
    XCTAssertEqual(result.speakerStatus, "unavailable")
    XCTAssertEqual(result.scopeID, "window")
  }

  func testIncompleteOrConflictingSnapshotsNeverProduceAttribution() {
    XCTAssertEqual(
      MeetingAXAdapter.project(
        bundleID: "us.zoom.xos", snapshot: .init(nodes: meeting().nodes, complete: false)),
      .unknown("incomplete_snapshot"))
    XCTAssertEqual(
      MeetingAXAdapter.project(
        bundleID: "us.zoom.xos", snapshot: meeting([.init(id: "audio", role: "AXButton")])),
      .unknown("duplicate_node_identity"))
  }

  func testGuestRoleSuffixDoesNotBecomePartOfParticipantName() {
    let snapshot = meeting([
      .init(id: "roster", parentID: "window", role: "AXOutline", description: "Participants list"),
      .init(id: "guest", parentID: "roster", role: "AXRow"),
      .init(id: "name", parentID: "guest", role: "AXStaticText", title: "Alex (Sam) (Guest)"),
    ])
    guard
      case .meeting(let result) = MeetingAXAdapter.project(
        bundleID: "us.zoom.xos", snapshot: snapshot)
    else {
      return XCTFail("Expected Zoom roster")
    }
    XCTAssertEqual(
      result.participants, [.init(id: "window/name", displayName: "Alex (Sam)", isLocal: false)])
    XCTAssertEqual(result.speakers, [])
  }
}
