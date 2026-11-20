import MeetingAccessibility
import XCTest

/// What Char's Join meeting offer reads: in a call, not in one, or can't tell.
final class CallStatusTests: XCTestCase {
  func meet(inCall: Bool, complete: Bool = true) -> MeetingAXSnapshot {
    var nodes: [MeetingAXNode] = [
      .init(id: "page", role: "AXWebArea", url: "https://meet.google.com/abc-defg-hij?authuser=0"),
      .init(id: "controls", parentID: "page", role: "AXGroup", description: "Call controls"),
      .init(id: "mic", parentID: "controls", role: "AXButton", title: "Turn off microphone"),
    ]
    if inCall {
      nodes.append(.init(id: "leave", parentID: "controls", role: "AXButton", title: "Leave call"))
    }
    return .init(nodes: nodes, complete: complete)
  }

  func testMeetCallCarriesItsOwnLink() {
    XCTAssertEqual(
      MeetingAXAdapter.callStatus(bundleID: "com.google.Chrome", snapshot: meet(inCall: true)),
      .init(
        platform: "google-meet", status: "in-call",
        callURL: "https://meet.google.com/abc-defg-hij?authuser=0"))
  }

  func testMeetLobbyIsNotInTheCall() {
    XCTAssertEqual(
      MeetingAXAdapter.callStatus(bundleID: "com.google.Chrome", snapshot: meet(inCall: false)),
      .init(platform: "google-meet", status: "not-in-call"))
  }

  func testAnIncompleteReadCannotSayNotInTheCall() {
    // No Accessibility permission reads as an empty, incomplete snapshot.
    XCTAssertEqual(
      MeetingAXAdapter.callStatus(
        bundleID: "com.google.Chrome", snapshot: .init(nodes: [], complete: false)),
      .init(platform: "google-meet", status: "unknown"))
    XCTAssertEqual(
      MeetingAXAdapter.callStatus(
        bundleID: "us.zoom.xos", snapshot: meet(inCall: true, complete: false)),
      .init(platform: "zoom", status: "unknown"))
  }

  func testZoomCallHasNoLink() {
    let snapshot = MeetingAXSnapshot(
      nodes: [
        .init(id: "window", role: "AXWindow", identifier: "zm.meeting.window.main"),
        .init(id: "audio", parentID: "window", role: "AXButton", identifier: "audio"),
        .init(id: "leave", parentID: "window", role: "AXButton", identifier: "leave"),
        .init(id: "people", parentID: "window", role: "AXButton", identifier: "participant"),
      ], complete: true)
    XCTAssertEqual(
      MeetingAXAdapter.callStatus(bundleID: "us.zoom.xos", snapshot: snapshot),
      .init(platform: "zoom", status: "in-call"))
  }

  /// The recorder's start: the one app in a call, and that call's link from the read that chose it.
  func testTheRecordedCallsLinkComesFromTheOneConfirmedCall() {
    let zoomInCall = MeetingAXSnapshot(
      nodes: [
        .init(id: "window", role: "AXWindow", identifier: "zm.meeting.window.main"),
        .init(id: "audio", parentID: "window", role: "AXButton", identifier: "audio"),
        .init(id: "leave", parentID: "window", role: "AXButton", identifier: "leave"),
        .init(id: "people", parentID: "window", role: "AXButton", identifier: "participant"),
      ], complete: true)
    let chosen = MeetingAXAdapter.soleCall([
      ("com.google.Chrome.beta", meet(inCall: false)),
      ("com.google.Chrome", meet(inCall: true)),
    ])
    XCTAssertEqual(chosen?.index, 1)
    XCTAssertEqual(chosen?.callURL, "https://meet.google.com/abc-defg-hij?authuser=0")
    // Two apps in calls: the recorder takes the whole Mac, and no call is named.
    XCTAssertNil(
      MeetingAXAdapter.soleCall([
        ("com.google.Chrome", meet(inCall: true)), ("us.zoom.xos", zoomInCall),
      ]))
    // Two Meet calls in one browser: it can't tell which one, so nothing is named.
    var two = meet(inCall: true).nodes
    two += [
      .init(id: "page2", role: "AXWebArea", url: "https://meet.google.com/xxx-yyyy-zzz"),
      .init(id: "controls2", parentID: "page2", role: "AXGroup", description: "Call controls"),
      .init(id: "mic2", parentID: "controls2", role: "AXButton", title: "Turn on microphone"),
      .init(id: "leave2", parentID: "controls2", role: "AXButton", title: "Leave call"),
    ]
    XCTAssertNil(
      MeetingAXAdapter.soleCall([("com.google.Chrome", .init(nodes: two, complete: true))]))
    XCTAssertNil(MeetingAXAdapter.soleCall([("com.google.Chrome", meet(inCall: false))]))
  }

  func testAppsItCannotReadHaveNoStatus() {
    XCTAssertNil(
      MeetingAXAdapter.callStatus(bundleID: "com.apple.Safari", snapshot: meet(inCall: true)))
  }
}
