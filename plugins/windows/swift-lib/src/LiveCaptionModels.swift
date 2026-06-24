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

    switch self {
    case .topCenter:
      return NSPoint(x: centerX, y: topY)
    case .topLeft:
      return NSPoint(x: leftX, y: topY)
    case .topRight:
      return NSPoint(x: rightX, y: topY)
    case .bottomLeft:
      return NSPoint(x: leftX, y: bottomY)
    case .bottomRight:
      return NSPoint(x: rightX, y: bottomY)
    case .bottomCenter:
      return NSPoint(x: centerX, y: bottomY)
    }
  }

}

struct LiveCaptionStatePayload: Codable {
  let text: String
  let opacity: Double
  let position: LiveCaptionPosition
  let minimized: Bool
}
