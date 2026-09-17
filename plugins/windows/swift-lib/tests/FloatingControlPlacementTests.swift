import Cocoa
import XCTest

@testable import swift_lib

final class FloatingControlPlacementTests: XCTestCase {
  func testControlsStayAtTheSameScreenPositionInBothSizesAndAtDisplayEdges() {
    let work = NSRect(x: -1920, y: 40, width: 1920, height: 1040)
    for (anchor, upwards) in [
      (NSPoint(x: -960, y: 71), true), (NSPoint(x: -960, y: 1028), false),
      (NSPoint(x: -64, y: 71), true),
    ] {
      for size in [
        NSSize(width: 111, height: 67), NSSize(width: 368, height: 459),
        NSSize(width: 111, height: 67),
      ] {
        let layout = FloatingControlPlacement.layout(
          anchor: anchor, size: size, workArea: work, expandsUpward: upwards)
        XCTAssertEqual(layout.frame.minX + layout.controlOffset.x, anchor.x)
        XCTAssertEqual(layout.frame.minY + layout.controlOffset.y, anchor.y)
        XCTAssertTrue(work.contains(layout.frame))
      }
    }
  }

  func testShortDisplaysReduceTheTranscriptWithoutMovingControls() {
    let anchor = NSPoint(x: 150, y: 140)
    let layout = FloatingControlPlacement.layout(
      anchor: anchor, size: NSSize(width: 368, height: 459),
      workArea: NSRect(x: 0, y: 0, width: 300, height: 320), expandsUpward: true)
    XCTAssertEqual(layout.frame, NSRect(x: 0, y: 117, width: 300, height: 203))
    XCTAssertEqual(layout.frame.minY + layout.controlOffset.y, anchor.y)
  }

  func testCollapseAfterDraggingUsesTheNewControlPosition() {
    let work = NSRect(x: 0, y: 0, width: 1920, height: 1080)
    let expanded = FloatingControlPlacement.layout(
      anchor: NSPoint(x: 1856, y: 71), size: NSSize(width: 368, height: 459), workArea: work,
      expandsUpward: true)
    let movedAnchor = NSPoint(x: expanded.frame.minX - 100 + expanded.controlOffset.x, y: 71)
    let compact = FloatingControlPlacement.layout(
      anchor: movedAnchor, size: NSSize(width: 144, height: 67), workArea: work, expandsUpward: true
    )
    XCTAssertEqual(compact.frame.midX, 1756)
  }
}
