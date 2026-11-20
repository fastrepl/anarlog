import CaptureCore
import MeetingAccessibility
import XCTest

final class TeamsAdapterTests: XCTestCase {
  // Teams personal desktop (teams.live.com) observed Nov 2, 2026, with sanitized names.
  // Work and school accounts load the same client from Microsoft's work hosts; no work-account
  // call has been read live, so these reuse the personal call's controls.
  func snapshot(inCall: Bool = true, url: String = "https://teams.live.com/v2/")
    -> MeetingAXSnapshot
  {
    var nodes: [MeetingAXNode] = [
      .init(id: "page", role: "AXWebArea", url: url),
      .init(id: "roster", parentID: "page", role: "AXOutline", description: "Attendees"),
      .init(
        id: "header", parentID: "roster", role: "AXRow", description: "In this meeting, 1 total"),
      .init(
        id: "header-text", parentID: "header", role: "AXStaticText", title: "In this meeting (1)"),
      .init(id: "row", parentID: "roster", role: "AXRow", description: "Alex, Organizer, Muted"),
      .init(id: "name", parentID: "row", role: "AXStaticText", title: "Alex"),
      .init(id: "role", parentID: "row", role: "AXStaticText", title: "Organizer"),
      .init(id: "chat", parentID: "page", role: "AXStaticText", title: "Meeting started"),
    ]
    if inCall {
      nodes += [
        .init(id: "controls", parentID: "page", role: "AXToolbar", description: "Meeting controls"),
        .init(id: "people", parentID: "controls", role: "AXButton", title: "People"),
        .init(id: "mic", parentID: "controls", role: "AXButton", title: "Unmute mic"),
        .init(id: "calling", parentID: "page", role: "AXGroup", description: "Calling controls"),
        .init(id: "leave", parentID: "calling", role: "AXButton", title: "Leave"),
      ]
    }
    return .init(nodes: nodes, complete: true)
  }

  func testRosterDoesNotTurnOrganizerOrMuteStateIntoLocalOrSpeakerIdentity() {
    guard
      case .meeting(let meeting) = MeetingAXAdapter.project(
        bundleID: "com.microsoft.teams2", snapshot: snapshot())
    else {
      return XCTFail("Expected Teams call")
    }
    XCTAssertEqual(
      meeting.participants, [.init(id: "page/name", displayName: "Alex", isLocal: nil)])
    XCTAssertEqual(meeting.speakers, [])
    XCTAssertEqual(meeting.platform, "teams")
  }

  func testChatHistoryAndRosterWithoutCallControlsCannotConfirmMeeting() {
    XCTAssertEqual(
      MeetingAXAdapter.project(bundleID: "com.microsoft.teams2", snapshot: snapshot(inCall: false)),
      .unknown("no_confirmed_meeting"))
  }

  func testWorkAndSchoolHostsConfirmACallOnlyByItsCallControls() {
    for url in ["https://teams.microsoft.com/v2/", "https://teams.cloud.microsoft/"] {
      guard
        case .meeting(let meeting) = MeetingAXAdapter.project(
          bundleID: "com.microsoft.teams2", snapshot: snapshot(url: url))
      else {
        XCTFail("Expected a Teams call on \(url)")
        continue
      }
      XCTAssertEqual(meeting.platform, "teams")
      XCTAssertEqual(meeting.speakers, [])
      XCTAssertEqual(meeting.speakerStatus, "unavailable")
      // A work account's chat, with the roster but no call controls, is no call.
      XCTAssertEqual(
        MeetingAXAdapter.project(
          bundleID: "com.microsoft.teams2", snapshot: snapshot(inCall: false, url: url)),
        .unknown("no_confirmed_meeting"))
    }
  }

  func testPreJoinScreenWithMicrophoneToggleIsNotACall() {
    // Before joining, the page offers its microphone and a way in, but no meeting toolbar or Leave.
    let preJoin = MeetingAXSnapshot(
      nodes: [
        .init(id: "page", role: "AXWebArea", url: "https://teams.microsoft.com/v2/"),
        .init(id: "mic", parentID: "page", role: "AXButton", title: "Unmute mic"),
        .init(id: "people", parentID: "page", role: "AXButton", title: "People"),
        .init(id: "join", parentID: "page", role: "AXButton", title: "Join now"),
      ], complete: true)
    XCTAssertEqual(
      MeetingAXAdapter.project(bundleID: "com.microsoft.teams2", snapshot: preJoin),
      .unknown("no_confirmed_meeting"))
  }

  func testOtherHostsAndPlainHTTPAreNotTeamsPages() {
    for url in [
      "http://teams.microsoft.com/v2/", "https://teams.microsoft.com.example.com/",
      "https://example.com/teams.live.com", "https://outlook.office.com/",
    ] {
      XCTAssertEqual(
        MeetingAXAdapter.project(bundleID: "com.microsoft.teams2", snapshot: snapshot(url: url)),
        .unknown("no_confirmed_meeting"), url)
    }
  }
}
