import CoreGraphics
import CoreImage
import Foundation

/// The built-in backgrounds (`WatermarkBackground.Scene`), drawn from code at the camera's size: no
/// photo or asset goes into them. Each is soft, like a room seen out of focus behind someone,
/// which also hides where the person's outline is unsure. None holds text, since meeting apps
/// mirror the person's own preview.
enum BackgroundScene {
  private static let lock = NSLock()
  nonisolated(unsafe) private static var cache: [WatermarkBackground.Scene: CIImage] = [:]

  /// `scene` at 1280x720, drawn once and kept.
  static func image(_ scene: WatermarkBackground.Scene) -> CIImage {
    lock.withLock {
      if let image = cache[scene] { return image }
      let image = draw(scene)
      cache[scene] = image
      return image
    }
  }

  private static func draw(_ scene: WatermarkBackground.Scene) -> CIImage {
    let size = CGSize(width: CameraIdentity.width, height: CameraIdentity.height)
    let space = CGColorSpace(name: CGColorSpace.sRGB)!
    guard
      let ctx = CGContext(
        data: nil, width: Int(size.width), height: Int(size.height), bitsPerComponent: 8,
        bytesPerRow: 0, space: space, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
    else { return CIImage(color: .gray).cropped(to: CGRect(origin: .zero, size: size)) }
    switch scene {
    case .studio: studio(ctx, size)
    case .dusk: dusk(ctx, size)
    case .daylight: daylight(ctx, size)
    }
    let drawn = ctx.makeImage().map { CIImage(cgImage: $0) } ?? CIImage()
    // Out of focus, as a real room behind someone is; the light patches need more.
    let sigma: Double = scene == .daylight ? 9 : 3
    let soft = drawn.clampedToExtent().applyingGaussianBlur(sigma: sigma)
      .cropped(to: CGRect(origin: .zero, size: size))
    // Kept as pixels, so each frame only blends it.
    let context = CIContext(options: [.cacheIntermediates: false])
    guard
      let pixels = context.createCGImage(soft, from: soft.extent, format: .RGBA8, colorSpace: space)
    else { return soft }
    return CIImage(cgImage: pixels)
  }

  private static func color(_ hex: UInt32, _ alpha: CGFloat = 1) -> CGColor {
    CGColor(
      srgbRed: CGFloat((hex >> 16) & 0xFF) / 255, green: CGFloat((hex >> 8) & 0xFF) / 255,
      blue: CGFloat(hex & 0xFF) / 255, alpha: alpha)
  }

  private static func linear(
    _ ctx: CGContext, _ stops: [(CGFloat, UInt32)], from start: CGPoint, to end: CGPoint
  ) {
    let gradient = CGGradient(
      colorsSpace: CGColorSpace(name: CGColorSpace.sRGB),
      colors: stops.map { color($0.1) } as CFArray,
      locations: stops.map(\.0))!
    ctx.drawLinearGradient(
      gradient, start: start, end: end,
      options: [.drawsBeforeStartLocation, .drawsAfterEndLocation])
  }

  private static func glow(
    _ ctx: CGContext, at center: CGPoint, radius: CGFloat, _ hex: UInt32, alpha: CGFloat
  ) {
    let gradient = CGGradient(
      colorsSpace: CGColorSpace(name: CGColorSpace.sRGB),
      colors: [color(hex, alpha), color(hex, 0)] as CFArray, locations: [0, 1])!
    ctx.drawRadialGradient(
      gradient, startCenter: center, startRadius: 0, endCenter: center, endRadius: radius,
      options: [])
  }

  /// A seamless paper backdrop: warm grey, brighter behind the person, with the floor's curve.
  private static func studio(_ ctx: CGContext, _ size: CGSize) {
    let (w, h) = (size.width, size.height)
    linear(
      ctx, [(0, 0x5E5953), (0.22, 0x77716A), (0.3, 0x8A847C), (1, 0x6F6962)],
      from: .zero, to: CGPoint(x: 0, y: h))
    glow(ctx, at: CGPoint(x: w * 0.5, y: h * 0.58), radius: w * 0.62, 0xE6DFD5, alpha: 0.85)
  }

  /// An evening sky, a low sun, and three ridges of hills fading into the haze.
  private static func dusk(_ ctx: CGContext, _ size: CGSize) {
    let (w, h) = (size.width, size.height)
    linear(
      ctx, [(0, 0xF6B98A), (0.3, 0xE88A6A), (0.55, 0x9B5E7E), (0.8, 0x4A4275), (1, 0x252B52)],
      from: CGPoint(x: 0, y: h * 0.25), to: CGPoint(x: 0, y: h))
    glow(ctx, at: CGPoint(x: w * 0.72, y: h * 0.4), radius: h * 0.45, 0xFFE2B0, alpha: 0.75)
    ctx.setFillColor(color(0xFFE9C4))
    ctx.fillEllipse(
      in: CGRect(x: w * 0.72 - h * 0.05, y: h * 0.4 - h * 0.05, width: h * 0.1, height: h * 0.1))
    let ridges: [(base: CGFloat, swing: CGFloat, phase: CGFloat, hex: UInt32)] = [
      (0.42, 0.05, 0.3, 0x8A5A78),
      (0.3, 0.07, 1.7, 0x5C3D62),
      (0.16, 0.06, 3.1, 0x33263F),
    ]
    for ridge in ridges {
      let path = CGMutablePath()
      path.move(to: .zero)
      for step in 0...64 {
        let x = CGFloat(step) / 64
        let y =
          ridge.base + ridge.swing * sin(x * 5.2 + ridge.phase) * 0.6
          + ridge.swing * sin(x * 11.7 + ridge.phase * 2) * 0.4
        path.addLine(to: CGPoint(x: x * w, y: y * h))
      }
      path.addLine(to: CGPoint(x: w, y: 0))
      path.closeSubpath()
      ctx.addPath(path)
      ctx.setFillColor(color(ridge.hex))
      ctx.fillPath()
    }
  }

  /// A pale plaster wall with afternoon light from a window off to the left.
  private static func daylight(_ ctx: CGContext, _ size: CGSize) {
    let (w, h) = (size.width, size.height)
    linear(
      ctx, [(0, 0xB3A795), (0.12, 0xC4B9A7), (0.14, 0xD3C8B6), (1, 0xC9BDAA)],
      from: .zero, to: CGPoint(x: 0, y: h))
    // Four panes of light, slanted as the sun comes in from high on the left.
    ctx.setFillColor(color(0xFFF1D2, 0.8))
    for (column, row) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
      let x = w * (0.12 + CGFloat(column) * 0.17) + CGFloat(row) * w * 0.09
      let y = h * (0.2 + CGFloat(row) * 0.3)
      let path = CGMutablePath()
      path.move(to: CGPoint(x: x, y: y))
      path.addLine(to: CGPoint(x: x + w * 0.15, y: y))
      path.addLine(to: CGPoint(x: x + w * 0.24, y: y + h * 0.27))
      path.addLine(to: CGPoint(x: x + w * 0.09, y: y + h * 0.27))
      path.closeSubpath()
      ctx.addPath(path)
    }
    ctx.fillPath()
  }
}
