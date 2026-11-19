import Cocoa
import XCTest

@testable import swift_lib

final class ClipboardSnapshotTests: XCTestCase {
  func testRestoringAgainPreservesEveryItemAndRepresentation() throws {
    let pasteboard = NSPasteboard.withUniqueName()
    defer { pasteboard.releaseGlobally() }

    let first = NSPasteboardItem()
    first.setString("Synthetic dictation clipboard", forType: .string)
    let customType = NSPasteboard.PasteboardType("so.anarlog.test.clipboard")
    let customData = Data([0, 1, 255])
    first.setData(customData, forType: customType)
    let second = NSPasteboardItem()
    second.setString("Second synthetic item", forType: .string)
    XCTAssertTrue(pasteboard.writeObjects([first, second]))
    let snapshot = try XCTUnwrap(ClipboardSnapshot(pasteboard))

    for _ in 0..<3 {
      pasteboard.clearContents()
      XCTAssertTrue(snapshot.write(to: pasteboard))
      let items = try XCTUnwrap(pasteboard.pasteboardItems)
      guard items.count == 2 else {
        XCTFail("Restoration lost clipboard items")
        return
      }
      XCTAssertEqual(items[0].string(forType: .string), "Synthetic dictation clipboard")
      XCTAssertEqual(items[0].data(forType: customType), customData)
      XCTAssertEqual(items[1].string(forType: .string), "Second synthetic item")
    }
  }
}
