import CoreGraphics
import Foundation
import ImageIO

/// What Anarlog Camera puts behind the person, in place of the meeting app's own virtual
/// background (which erases the watermark along with the room): nothing, a blur of the room, one
/// of the built-in scenes, or the person's own image. One plain JSON value that Anarlog writes into
/// the agent's `state.json`; `plugins/camera-watermark` holds the same rules.
///
/// ```json
/// {"kind":"image","image":"dusk","custom":null}
/// ```
///
/// `image` is a scene or `custom`; `custom` is the person's own image as a JPEG or PNG data URL,
/// kept while a scene is chosen. A bad or missing field falls back on its own: an unknown kind is
/// none, an unknown scene is Studio, and a custom image that doesn't decode draws Studio, so a
/// person who asked to hide their room never sees it come back over a bad value.
public enum WatermarkBackground: Equatable, Sendable {
  case none
  case blur
  case scene(Scene)
  case image(CGImage)

  /// Built-in backgrounds, drawn by `BackgroundScene` from code: no photos, nothing licensed.
  public enum Scene: String, CaseIterable, Sendable {
    /// A warm grey seamless backdrop, lit from the middle.
    case studio
    /// An evening sky over layered hills.
    case dusk
    /// A pale wall with soft window light falling across it.
    case daylight
  }

  /// Longest image data URL Anarlog Camera reads; a 1280x720 JPEG is well under it.
  public static let maxImageDataURLLength = 1_500_000
  /// Largest side of an image Anarlog Camera draws behind the person.
  public static let maxImagePixels = 4096

  public static func == (a: WatermarkBackground, b: WatermarkBackground) -> Bool {
    switch (a, b) {
    case (.none, .none), (.blur, .blur): return true
    case (.scene(let x), .scene(let y)): return x == y
    case (.image(let x), .image(let y)): return x === y
    default: return false
    }
  }

  /// The kind the agent reports in its status as `background`: what it asks each new frame to
  /// draw. Frames already on their way may still show the last one, so Anarlog waits for
  /// `ProtectedOutput`'s receipt before turning a meeting app's own effect off.
  public var kind: String {
    switch self {
    case .none: return "none"
    case .blur: return "blur"
    case .scene, .image: return "image"
    }
  }

  public static func decode(_ value: Any?) -> WatermarkBackground? {
    guard let object = value as? [String: Any] else { return nil }
    switch object["kind"] as? String {
    case "blur":
      return .blur
    case "image":
      if object["image"] as? String == "custom",
        let image = (object["custom"] as? String).flatMap(decodeImage)
      {
        return .image(image)
      }
      return .scene((object["image"] as? String).flatMap(Scene.init(rawValue:)) ?? .studio)
    default:
      return WatermarkBackground.none
    }
  }

  /// A `data:image/jpeg;base64,` or `data:image/png;base64,` URL, decoded; nil for
  /// anything else or too large.
  static func decodeImage(_ dataURL: String) -> CGImage? {
    guard dataURL.count <= maxImageDataURLLength,
      let prefix = ["data:image/jpeg;base64,", "data:image/png;base64,"].first(
        where: dataURL.hasPrefix),
      let data = Data(base64Encoded: String(dataURL.dropFirst(prefix.count))),
      let source = CGImageSourceCreateWithData(data as CFData, nil),
      let image = CGImageSourceCreateImageAtIndex(source, 0, nil),
      image.width <= maxImagePixels, image.height <= maxImagePixels
    else { return nil }
    return image
  }
}
