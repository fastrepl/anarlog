import Cocoa
import XCTest

@testable import swift_lib

final class FloatingBarPlacementTests: XCTestCase {
  private let workArea = NSRect(x: -1920, y: 40, width: 1920, height: 1040)
  private let compactSize = NSSize(width: 111, height: 67)
  private let expandedSize = NSSize(width: 368, height: 459)

  func testBottomExpansionAndCollapsePreserveBottomCenter() {
    let compact = NSRect(x: -1015.5, y: 48, width: 111, height: 67)
    XCTAssertTrue(FloatingBarPlacement.expandsUpward(frame: compact, workArea: workArea))
    let expanded = FloatingBarPlacement.resizedFrame(
      compact, size: expandedSize, workArea: workArea, expandsUpward: true)
    XCTAssertEqual(expanded.midX, compact.midX)
    XCTAssertEqual(expanded.minY, compact.minY)
    XCTAssertEqual(
      FloatingBarPlacement.resizedFrame(
        expanded, size: compactSize, workArea: workArea, expandsUpward: true), compact)
  }

  func testTopExpansionAndCollapsePreserveTopCenter() {
    let compact = NSRect(x: -1015.5, y: 1005, width: 111, height: 67)
    XCTAssertFalse(FloatingBarPlacement.expandsUpward(frame: compact, workArea: workArea))
    let expanded = FloatingBarPlacement.resizedFrame(
      compact, size: expandedSize, workArea: workArea, expandsUpward: false)
    XCTAssertEqual(expanded.midX, compact.midX)
    XCTAssertEqual(expanded.maxY, compact.maxY)
    XCTAssertEqual(
      FloatingBarPlacement.resizedFrame(
        expanded, size: compactSize, workArea: workArea, expandsUpward: false), compact)
  }

  func testExpansionClampsAtDisplayEdge() {
    let frame = NSRect(x: -112, y: 48, width: 111, height: 67)
    let expanded = FloatingBarPlacement.resizedFrame(
      frame, size: expandedSize, workArea: workArea, expandsUpward: true)
    XCTAssertEqual(expanded.maxX, workArea.maxX)
    XCTAssertTrue(workArea.contains(expanded))
    XCTAssertEqual(
      FloatingBarPlacement.resizedFrame(
        expanded, size: compactSize, workArea: workArea, expandsUpward: true,
        expansion: (frame, expanded)), frame)
  }
}
