import Cocoa

enum FloatingBarPlacement {
  static func expandsUpward(frame: NSRect, workArea: NSRect) -> Bool {
    workArea.maxY - frame.maxY > frame.minY - workArea.minY
  }

  static func resizedFrame(
    _ frame: NSRect, size: NSSize, workArea: NSRect, expandsUpward: Bool
  ) -> NSRect {
    let x = frame.midX - size.width / 2
    let y = expandsUpward ? frame.minY : frame.maxY - size.height
    return NSRect(
      x: min(max(x, workArea.minX), max(workArea.minX, workArea.maxX - size.width)),
      y: min(max(y, workArea.minY), max(workArea.minY, workArea.maxY - size.height)),
      width: size.width,
      height: size.height)
  }
}
