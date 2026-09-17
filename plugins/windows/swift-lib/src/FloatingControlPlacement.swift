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
    let minHeight = min(
      size.height, FloatingBarLayout.containerSize(isExpanded: false, showsExpand: true).height,
      workArea.height)
    let y: CGFloat
    let height: CGFloat
    if expandsUpward {
      y = max(
        workArea.minY,
        min(
          anchor.y - FloatingBarLayout.inset - FloatingBarLayout.compactHeight / 2,
          workArea.maxY - minHeight))
      height = min(size.height, workArea.maxY - y)
    } else {
      let top =
        min(
          workArea.maxY,
          max(
            workArea.minY + minHeight,
            anchor.y + FloatingBarLayout.inset + FloatingBarLayout.hoverHandleReservedHeight
              + FloatingBarLayout.compactHeight / 2))
      height = min(size.height, top - workArea.minY)
      y = top - height
    }
    let x = min(max(anchor.x - width / 2, workArea.minX), workArea.maxX - width)
    return Layout(
      frame: NSRect(x: x, y: y, width: width, height: height),
      controlsCenterX: anchor.x - x, expandsUpward: expandsUpward)
  }
}
