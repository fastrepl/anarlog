import CaptureCore
import MeetingAccessibility
import XCTest

final class WebexAdapterTests: XCTestCase {
  // Webex 46.11, a solo meeting observed Nov 16, 2026, with sanitized names.
  func snapshot(inCall: Bool = true) -> MeetingAXSnapshot {
    var nodes: [MeetingAXNode] = [
      .init(id: "window", role: "AXWindow", title: "Alex's meeting", identifier: "PopOutWindow"),
      .init(
        id: "audio", parentID: "window", role: "AXButton", title: "Unmute",
        identifier: "callControl_audio"),
      .init(id: "list", parentID: "window", role: "AXOutline", description: "Participants"),
      .init(id: "group-row", parentID: "list", role: "AXRow"),
      .init(
        id: "group", parentID: "group-row", role: "AXUnknown",
        description: " Participants  group, 2 participant(s), button, expanded",
        identifier: "GenericParticipantListGroupCellView"),
      .init(id: "group-title", parentID: "group", role: "AXStaticText", title: "Participants (2)"),
      .init(id: "row", parentID: "list", role: "AXRow"),
      .init(
        id: "cell", parentID: "row", role: "AXUnknown",
        description: "Alex, Host, presenter, me, Muted, ",
        identifier: "GenericParticipantListCellView"),
      .init(id: "name", parentID: "cell", role: "AXStaticText", title: "Alex"),
      .init(id: "roles", parentID: "cell", role: "AXStaticText", title: "Host, presenter, me"),
      .init(id: "row2", parentID: "list", role: "AXRow"),
      .init(
        id: "cell2", parentID: "row2", role: "AXUnknown", description: "Sam, Muted, ",
        identifier: "GenericParticipantListCellView"),
      .init(id: "name2", parentID: "cell2", role: "AXStaticText", title: "Sam"),
    ]
    nodes.append(
      inCall
        ? .init(
          id: "end", parentID: "window", role: "AXButton", title: "End meeting",
          identifier: "callControl_end")
        : .init(
          id: "start", parentID: "window", role: "AXButton", title: "Start meeting",
          identifier: "interstitial_joinMeeting_button"))
    return .init(nodes: nodes, complete: true)
  }

  func testCallWindowGivesRosterWithTheLocalPerson() {
    guard
      case .meeting(let meeting) = MeetingAXAdapter.project(
        bundleID: "Cisco-Systems.Spark", snapshot: snapshot())
    else {
      return XCTFail("Expected Webex call")
    }
    XCTAssertEqual(meeting.platform, "webex")
    XCTAssertEqual(
      meeting.participants,
      [
        .init(id: "window/name", displayName: "Alex", isLocal: true),
        .init(id: "window/name2", displayName: "Sam", isLocal: false),
      ])
    XCTAssertTrue(meeting.rosterVisible)
    XCTAssertEqual(meeting.participantCount, 2)
    XCTAssertEqual(meeting.speakers, [])
  }

  /// The helper sends this line to Anarlog's adapter, which reads these keys and no others.
  func testEncodesTheRosterAnarlogReads() throws {
    guard
      case .meeting(let meeting) = MeetingAXAdapter.project(
        bundleID: "Cisco-Systems.Spark", snapshot: snapshot())
    else {
      return XCTFail("Expected Webex call")
    }
    let line = try XCTUnwrap(EventLine.parse(EventLine.meetingObservation(meeting).text))
    XCTAssertEqual(line.event, "meeting-observation")
    XCTAssertEqual(line.value("platform"), "webex")
    XCTAssertEqual(line.value("rosterVisible"), "true")
    XCTAssertEqual(line.value("participantCount"), "2")
    XCTAssertEqual(line.value("speakerStatus"), "unavailable")
    let rows = line.fields.filter { $0.key.hasPrefix("participant.") }.map(\.value)
    XCTAssertEqual(rows, ["window/name", "Alex", "true", "window/name2", "Sam", "false"])
    XCTAssertFalse(line.fields.contains { $0.key.hasPrefix("speaker.") })
  }

  func testASecondGroupLeavesTheCallUncounted() {
    // A lobby's own group would count people the call's rows don't hold.
    var nodes = snapshot().nodes
    nodes += [
      .init(id: "lobby-row", parentID: "list", role: "AXRow"),
      .init(
        id: "lobby", parentID: "lobby-row", role: "AXUnknown",
        description: " Waiting  group, 1 participant(s), button, expanded",
        identifier: "GenericParticipantListGroupCellView"),
    ]
    guard
      case .meeting(let meeting) = MeetingAXAdapter.project(
        bundleID: "Cisco-Systems.Spark", snapshot: .init(nodes: nodes, complete: true))
    else {
      return XCTFail("Expected Webex call")
    }
    XCTAssertNil(meeting.participantCount)
  }

  func testPreJoinScreenIsNotACall() {
    XCTAssertEqual(
      MeetingAXAdapter.project(bundleID: "Cisco-Systems.Spark", snapshot: snapshot(inCall: false)),
      .unknown("no_confirmed_meeting"))
    XCTAssertTrue(MeetingAXAdapter.supports(bundleID: "Cisco-Systems.Spark"))
    XCTAssertEqual(
      MeetingAXAdapter.callStatus(
        bundleID: "Cisco-Systems.Spark", snapshot: snapshot(inCall: false))?.status,
      "not-in-call")
  }
}
