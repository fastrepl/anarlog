import Cocoa

enum LiveCaptionPosition: String, Codable, CaseIterable {
  case topCenter
  case topLeft
  case topRight
  case bottomLeft
  case bottomRight
  case bottomCenter

  var title: String {
    switch self {
    case .topCenter:
      return "Top"
    case .topLeft:
      return "Top left"
    case .topRight:
      return "Top right"
    case .bottomLeft:
      return "Bottom left"
    case .bottomRight:
      return "Bottom right"
    case .bottomCenter:
      return "Bottom"
    }
  }

  func origin(in frame: NSRect, size: NSSize) -> NSPoint {
    let margin = LiveCaptionLayout.screenMargin
    let topY = frame.maxY - size.height - LiveCaptionLayout.topOffset
    let bottomY = frame.minY + margin
    let centerX = frame.midX - size.width / 2
    let leftX = frame.minX + margin
    let rightX = frame.maxX - size.width - margin

    let origin: NSPoint
    switch self {
    case .topCenter:
      origin = NSPoint(x: centerX, y: topY)
    case .topLeft:
      origin = NSPoint(x: leftX, y: topY)
    case .topRight:
      origin = NSPoint(x: rightX, y: topY)
    case .bottomLeft:
      origin = NSPoint(x: leftX, y: bottomY)
    case .bottomRight:
      origin = NSPoint(x: rightX, y: bottomY)
    case .bottomCenter:
      origin = NSPoint(x: centerX, y: bottomY)
    }

    let minX = frame.minX + margin
    let maxX = max(minX, frame.maxX - margin - size.width)
    let minY = frame.minY + margin
    let maxY = max(minY, frame.maxY - margin - size.height)

    return NSPoint(
      x: min(max(origin.x, minX), maxX),
      y: min(max(origin.y, minY), maxY))
  }

}

struct LiveCaptionStatePayload: Codable {
  let text: String
  let opacity: Double
  let position: LiveCaptionPosition
  let minimized: Bool
}
