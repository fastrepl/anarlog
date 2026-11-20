import CaptureCore
import Foundation
import XCTest

final class MeetingObservationTests: XCTestCase {
  func testObservationsCannotCrossPauseResumeOrStopBoundaries() throws {
    var timeline = try CaptureTimeline(origin: 10)
    XCTAssertFalse(timeline.acceptsObservation(startedAt: 9))
    XCTAssertTrue(timeline.acceptsObservation(startedAt: 11))
    try timeline.pause(at: 12)
    XCTAssertFalse(timeline.acceptsObservation(startedAt: 11))
    XCTAssertFalse(timeline.acceptsObservation(startedAt: 13))
    try timeline.resume(at: 14)
    XCTAssertFalse(timeline.acceptsObservation(startedAt: 13))
    XCTAssertTrue(timeline.acceptsObservation(startedAt: 15))
    XCTAssertFalse(timeline.acceptsObservation(startedAt: .nan))
    try timeline.stop(at: 16)
    XCTAssertFalse(timeline.acceptsObservation(startedAt: 17))
  }

  func testLiveReadsRefreshRosterWhileJournalDeduplicatesAndPauseBoundariesHold() throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: false)
    defer { try? FileManager.default.removeItem(at: directory) }
    let collector = LiveOutputTests.Collector()
    let output = try AudioOutput(
      directory: directory, captureID: "capture", processID: 1, bundleID: "us.zoom.xos",
      microphoneID: nil, live: LiveOutput(write: collector.write))
    let meeting = MeetingObservation(
      platform: "zoom", scopeID: "call",
      participants: [
        .init(id: "occurrence-1", displayName: "Alex", isLocal: true)
      ], speakers: [], speakerStatus: "unavailable")
    output.observeMeeting(meeting, unknownReason: nil, startedAt: captureHostTime())
    output.observeMeeting(meeting, unknownReason: nil, startedAt: captureHostTime())
    output.observeMeeting(nil, unknownReason: "incomplete_snapshot", startedAt: captureHostTime())
    let beforePause = captureHostTime()
    try output.pause()
    output.observeMeeting(meeting, unknownReason: nil, startedAt: captureHostTime())
    try output.resume()
    output.observeMeeting(meeting, unknownReason: nil, startedAt: beforePause)
    output.observeMeeting(meeting, unknownReason: nil, startedAt: captureHostTime())
    try output.end()
    output.observeMeeting(nil, unknownReason: "closed", startedAt: captureHostTime())
    _ = try output.finish(reason: "test")
    XCTAssertEqual(
      collector.events.filter { $0.event == "meeting-observation" }.count, 3,
      "Every accepted read refreshes speaker evidence, including an unchanged roster")
    let events = try String(
      contentsOf: directory.appendingPathComponent("capture.journal"), encoding: .utf8
    )
    .split(separator: "\n").compactMap { EventLine.parse(String($0)) }
    XCTAssertEqual(
      events.map(\.event),
      [
        "prepared", "meeting-observation", "meeting-observation-unavailable", "paused", "resumed",
        "meeting-observation", "stopping", "ended",
      ])
  }
}
