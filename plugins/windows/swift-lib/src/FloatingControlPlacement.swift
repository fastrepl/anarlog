import Cocoa

enum FloatingControlPlacement {
  struct Layout: Equatable {
    let frame: NSRect
    let controlsCenterX: CGFloat
    let expandsUpward: Bool

    var controlOffset: NSPoint {
      NSPoint(
        x: controlsCenterX,
        y: expandsUpward
          ? FloatingBarLayout.inset + FloatingBarLayout.compactHeight / 2
          : frame.height - FloatingBarLayout.inset - FloatingBarLayout.hoverHandleReservedHeight
            - FloatingBarLayout.compactHeight / 2)
    }
  }

  static func layout(anchor: NSPoint, size: NSSize, workArea: NSRect, expandsUpward: Bool) -> Layout
  {
    let width = min(size.width, workArea.width)
    let y: CGFloat
    let height: CGFloat
    if expandsUpward {
      y = anchor.y - FloatingBarLayout.inset - FloatingBarLayout.compactHeight / 2
      height = min(size.height, workArea.maxY - y)
    } else {
      let top =
        anchor.y + FloatingBarLayout.inset + FloatingBarLayout.hoverHandleReservedHeight
        + FloatingBarLayout.compactHeight / 2
      height = min(size.height, top - workArea.minY)
      y = top - height
    }
    let x = min(max(anchor.x - width / 2, workArea.minX), workArea.maxX - width)
    return Layout(
      frame: NSRect(x: x, y: y, width: width, height: height),
      controlsCenterX: anchor.x - x, expandsUpward: expandsUpward)
  }
}
