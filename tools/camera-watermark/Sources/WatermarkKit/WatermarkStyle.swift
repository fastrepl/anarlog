import CoreGraphics
import Foundation
import ImageIO

/// What Anarlog Camera draws while Anarlog records: one plain JSON value in the agent's
/// `state.json`. Every field is optional on the wire and clamped here, so a bad value draws the
/// layout's default instead of failing. Anarlog writes no style today, which draws `badge`.
///
/// ```json
/// {"layout":"badge","position":"bottom_left","text":"is taking notes","color":"#F04545",
///  "opacity":1,"image":null}
/// ```
public struct WatermarkStyle: Equatable, Sendable {
  public enum Layout: String, CaseIterable, Sendable {
    /// The logo and the text on a dark pill in a corner.
    case badge
    /// A soft glow around the frame's edge, with a tag in a corner.
    case glow
    /// The logo alone in a corner, faint.
    case mark
    /// A band across the bottom or top edge with the logo and the text.
    case banner
  }

  public enum Position: String, CaseIterable, Sendable {
    case topLeft = "top_left"
    case topRight = "top_right"
    case bottomLeft = "bottom_left"
    case bottomRight = "bottom_right"

    public var isTop: Bool { self == .topLeft || self == .topRight }
    public var isLeft: Bool { self == .topLeft || self == .bottomLeft }
  }

  public static let defaultText = "is taking notes"
  public static let maxTextLength = 32
  public static let minOpacity: CGFloat = 0.3
  /// The person's image arrives as a PNG data URL; longer ones are dropped.
  public static let maxImageDataURLLength = 400_000
  public static let maxImagePixels = 1024

  public var layout: Layout
  public var position: Position
  public var text: String
  /// `#RRGGBB`: the badge and banner dot, the glow, the mark's ink.
  public var color: String
  public var opacity: CGFloat
  /// The person's own logo, drawn where Anarlog's wordmark would be.
  public var image: CGImage?

  public init(
    layout: Layout, position: Position = .bottomLeft, text: String = WatermarkStyle.defaultText,
    color: String, opacity: CGFloat = 1, image: CGImage? = nil
  ) {
    self.layout = layout
    self.position = position
    self.text = Self.clampText(text)
    self.color = Self.clampColor(color) ?? Self.defaultColor(layout)
    self.opacity = Self.clampOpacity(opacity)
    self.image = image
  }

  public static func == (a: WatermarkStyle, b: WatermarkStyle) -> Bool {
    a.layout == b.layout && a.position == b.position && a.text == b.text && a.color == b.color
      && a.opacity == b.opacity && a.image === b.image
  }

  // MARK: Presets

  public static let badge = WatermarkStyle(layout: .badge, color: "#F04545")
  public static let glow = WatermarkStyle(layout: .glow, position: .bottomRight, color: "#5B8DEF")
  public static let mark = WatermarkStyle(
    layout: .mark, position: .bottomRight, color: "#FFFFFF", opacity: 0.75)
  public static let banner = WatermarkStyle(layout: .banner, color: "#F04545")

  public static func preset(_ name: String) -> WatermarkStyle? {
    switch name {
    case "badge": return .badge
    case "glow": return .glow
    case "mark": return .mark
    case "banner": return .banner
    default: return nil
    }
  }

  static func defaultColor(_ layout: Layout) -> String {
    switch layout {
    case .badge, .banner: return "#F04545"
    case .glow: return "#5B8DEF"
    case .mark: return "#FFFFFF"
    }
  }

  // MARK: Wire

  /// Reads the JSON object; unknown or bad fields fall back. Nil only when it isn't an object.
  public static func decode(_ value: Any?) -> WatermarkStyle? {
    guard let object = value as? [String: Any] else { return nil }
    let layout = (object["layout"] as? String).flatMap(Layout.init(rawValue:)) ?? .badge
    let position = (object["position"] as? String).flatMap(Position.init(rawValue:)) ?? .bottomLeft
    let opacity = (object["opacity"] as? NSNumber).map { CGFloat($0.doubleValue) } ?? 1
    return WatermarkStyle(
      layout: layout, position: position, text: object["text"] as? String ?? "",
      color: object["color"] as? String ?? "", opacity: opacity,
      image: (object["image"] as? String).flatMap(decodeImage))
  }

  /// Collapses whitespace and cuts to `maxTextLength`; blank is the default copy.
  static func clampText(_ raw: String) -> String {
    let words = raw.split(whereSeparator: { $0.isWhitespace || $0.isNewline })
    let text = String(words.joined(separator: " ").prefix(maxTextLength))
      .trimmingCharacters(in: .whitespaces)
    return text.isEmpty ? defaultText : text
  }

  static func clampColor(_ raw: String) -> String? {
    let hex = raw.trimmingCharacters(in: .whitespaces).uppercased()
    guard hex.count == 7, hex.first == "#",
      hex.dropFirst().allSatisfy({ $0.isHexDigit })
    else { return nil }
    return hex
  }

  static func clampOpacity(_ raw: CGFloat) -> CGFloat {
    guard raw.isFinite else { return 1 }
    return (min(1, max(minOpacity, raw)) * 100).rounded() / 100
  }

  /// A `data:image/png;base64,` URL, decoded; nil for anything else or too large.
  static func decodeImage(_ dataURL: String) -> CGImage? {
    let prefix = "data:image/png;base64,"
    guard dataURL.hasPrefix(prefix), dataURL.count <= maxImageDataURLLength,
      let data = Data(base64Encoded: String(dataURL.dropFirst(prefix.count))),
      let source = CGImageSourceCreateWithData(data as CFData, nil),
      let image = CGImageSourceCreateImageAtIndex(source, 0, nil),
      image.width <= maxImagePixels, image.height <= maxImagePixels
    else { return nil }
    return image
  }

  var cgColor: CGColor {
    let value = UInt32(color.dropFirst(), radix: 16) ?? 0xFFFFFF
    return CGColor(
      srgbRed: CGFloat((value >> 16) & 0xFF) / 255, green: CGFloat((value >> 8) & 0xFF) / 255,
      blue: CGFloat(value & 0xFF) / 255, alpha: 1)
  }

  /// Relative luminance of `color`, to pick dark or light ink on top of it.
  var colorIsLight: Bool {
    let value = UInt32(color.dropFirst(), radix: 16) ?? 0xFFFFFF
    let r = Double((value >> 16) & 0xFF) / 255
    let g = Double((value >> 8) & 0xFF) / 255
    let b = Double(value & 0xFF) / 255
    return 0.2126 * r + 0.7152 * g + 0.0722 * b > 0.6
  }
}
