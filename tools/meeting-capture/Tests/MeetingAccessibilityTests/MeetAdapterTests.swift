import CaptureCore
import MeetingAccessibility
import XCTest

final class MeetAdapterTests: XCTestCase {
  // Chrome/Meet's English accessibility structure observed Nov 2, 2026; names/URL sanitized.
  func snapshot(url: String = "https://meet.google.com/meet/abc-defg-hij", inCall: Bool = true)
    -> MeetingAXSnapshot
  {
    var nodes: [MeetingAXNode] = [
      .init(id: "page", role: "AXWebArea", url: url),
      .init(id: "controls", parentID: "page", role: "AXGroup", description: "Call controls"),
      .init(id: "mic", parentID: "controls", role: "AXButton", title: "Turn on microphone"),
      .init(id: "people", parentID: "page", role: "AXList", description: "Participants"),
      .init(id: "row", parentID: "people", role: "AXGroup", description: "Alex"),
      .init(id: "name", parentID: "row", role: "AXStaticText", title: "Alex"),
      .init(id: "you", parentID: "row", role: "AXStaticText", title: "(You)"),
      .init(id: "host", parentID: "row", role: "AXStaticText", title: "Meeting host"),
      .init(id: "chat", parentID: "page", role: "AXStaticText", title: "Other person"),
    ]
    if inCall {
      nodes.append(.init(id: "leave", parentID: "controls", role: "AXButton", title: "Leave call"))
    }
    return .init(nodes: nodes, complete: true)
  }

  func testProjectsRosterOnlyInsideVerifiedMeetCall() {
    guard
      case .meeting(let meeting) = MeetingAXAdapter.project(
        bundleID: "com.google.Chrome", snapshot: snapshot())
    else {
      return XCTFail("Expected Meet call")
    }
    XCTAssertEqual(
      meeting.participants, [.init(id: "page/name", displayName: "Alex", isLocal: true)])
    XCTAssertEqual(meeting.speakers, [])
    XCTAssertEqual(meeting.platform, "google-meet")
  }

  // Meet in another Chromium browser records that browser's audio, not the whole Mac's.
  // Untested live: only Chrome is installed on the Mac this was written on.
  func testRecognizesMeetInEveryChromiumBrowser() {
    for bundleID in [
      "com.google.Chrome", "com.brave.Browser", "com.microsoft.edgemac",
      "company.thebrowser.dia", "com.vivaldi.Vivaldi",
      "com.operasoftware.Opera", "org.chromium.Chromium",
    ] {
      XCTAssertTrue(MeetingAXAdapter.supports(bundleID: bundleID), bundleID)
      guard
        case .meeting(let meeting) = MeetingAXAdapter.project(
          bundleID: bundleID, snapshot: snapshot())
      else { return XCTFail("Expected a Meet call in \(bundleID)") }
      XCTAssertEqual(meeting.platform, "google-meet")
    }
  }

  func testIgnoresUnsupportedBrowsers() {
    for bundleID in ["com.apple.Safari", "org.mozilla.firefox"] {
      XCTAssertFalse(MeetingAXAdapter.supports(bundleID: bundleID), bundleID)
      XCTAssertEqual(
        MeetingAXAdapter.project(bundleID: bundleID, snapshot: snapshot()),
        .unknown("unsupported_application"))
    }
  }

  func testRejectsPrejoinOtherWebsitesAndMeetHome() {
    for url in [
      "https://example.com/abc-defg-hij", "https://meet.google.com.example.com/abc-defg-hij",
      "https://meet.google.com/home", "http://meet.google.com/abc-defg-hij",
    ] {
      XCTAssertEqual(
        MeetingAXAdapter.project(bundleID: "com.google.Chrome", snapshot: snapshot(url: url)),
        .unknown("no_confirmed_meeting"))
    }
    XCTAssertEqual(
      MeetingAXAdapter.project(bundleID: "com.google.Chrome", snapshot: snapshot(inCall: false)),
      .unknown("no_confirmed_meeting"))
  }
}
