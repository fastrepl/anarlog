import CoreGraphics
import CoreText
import CoreVideo
import Foundation

/// Where each part of a style lands in a frame, in a context whose y points up. Every size is a
/// fraction of the frame's height, so a style reads the same at 360p and 1080p and in a meeting
/// app's small tile; everything sits along the frame's edges, clear of the person's face.
public struct WatermarkLayout: Equatable {
  /// The dark pill (badge), the band (banner) or the coloured tag (glow). None for the mark.
  public var plate: CGRect?
  public var dot: CGRect?
  /// Anarlog's wordmark, or the person's image.
  public var logo: CGRect
  public var text: CGRect?
  /// How far the glow reaches in from the frame's edge.
  public var glowWidth: CGFloat?
  /// The line that draws `text`, cut to fit.
  let line: CTLine?

  public static func == (a: WatermarkLayout, b: WatermarkLayout) -> Bool {
    a.plate == b.plate && a.dot == b.dot && a.logo == b.logo && a.text == b.text
      && a.glowWidth == b.glowWidth
  }

  /// Every rect something is drawn in, for checking what a style covers.
  public var covered: [CGRect] { [plate, dot, logo, text].compactMap { $0 } }

  /// Widest a badge or tag gets, as a share of the frame's width; longer text is cut.
  public static let maxPlateWidth: CGFloat = 0.6

  public static func make(_ style: WatermarkStyle, frame: CGSize) -> WatermarkLayout {
    let unit = frame.height
    let margin = (unit * 0.04).rounded()
    switch style.layout {
    case .badge:
      return row(style, frame: frame, height: (unit * 0.09).rounded(), inset: margin, dot: true)
    case .glow:
      var layout = row(
        style, frame: frame, height: (unit * 0.085).rounded(), inset: margin, dot: false)
      layout.glowWidth = (unit * 0.06).rounded()
      return layout
    case .banner:
      return banner(style, frame: frame, margin: margin)
    case .mark:
      let size = logoSize(
        style, height: unit * (style.image == nil ? 0.075 : 0.1), maxWidth: unit * 0.4)
      let origin = corner(size, in: frame, position: style.position, inset: margin)
      return WatermarkLayout(
        plate: nil, dot: nil, logo: CGRect(origin: origin, size: size), text: nil, glowWidth: nil,
        line: nil)
    }
  }

  /// A pill in a corner: [dot] logo text.
  private static func row(
    _ style: WatermarkStyle, frame: CGSize, height: CGFloat, inset: CGFloat, dot hasDot: Bool
  ) -> WatermarkLayout {
    let padding = height * 0.42
    let gap = height * 0.26
    let dotSize = height * 0.22
    let logo = logoSize(
      style, height: height * (style.image == nil ? 0.42 : 0.62), maxWidth: height * 3)
    let fixed = padding * 2 + (hasDot ? dotSize + gap : 0) + logo.width + gap * 0.8
    let maxWidth = frame.width * maxPlateWidth
    let line = Watermark.textLine(
      style.text, size: height * 0.4, color: Watermark.ink(style), maxWidth: maxWidth - fixed)
    let textWidth = ceil(CGFloat(CTLineGetTypographicBounds(line, nil, nil, nil)))
    let plateSize = CGSize(width: fixed + textWidth, height: height)
    let plate = CGRect(
      origin: corner(plateSize, in: frame, position: style.position, inset: inset), size: plateSize)

    var x = plate.minX + padding
    var dotRect: CGRect?
    if hasDot {
      dotRect = CGRect(x: x, y: plate.midY - dotSize / 2, width: dotSize, height: dotSize)
      x += dotSize + gap
    }
    let logoRect = CGRect(
      x: x, y: plate.midY - logo.height / 2, width: logo.width, height: logo.height)
    x += logo.width + gap * 0.8
    let text = CGRect(x: x, y: plate.minY, width: textWidth, height: height)
    return WatermarkLayout(
      plate: plate, dot: dotRect, logo: logoRect, text: text, glowWidth: nil, line: line)
  }

  /// A band across the top or bottom edge; its content keeps to the chosen side.
  private static func banner(_ style: WatermarkStyle, frame: CGSize, margin: CGFloat)
    -> WatermarkLayout
  {
    let height = (frame.height * 0.12).rounded()
    let plate = CGRect(
      x: 0, y: style.position.isTop ? frame.height - height : 0, width: frame.width, height: height)
    let gap = height * 0.22
    let dotSize = height * 0.17
    let logo = logoSize(
      style, height: height * (style.image == nil ? 0.34 : 0.5), maxWidth: height * 3)
    let fixed = dotSize + gap + logo.width + gap * 0.8
    let line = Watermark.textLine(
      style.text, size: height * 0.34, color: Watermark.ink(style),
      maxWidth: frame.width - margin * 2 - fixed)
    let textWidth = ceil(CGFloat(CTLineGetTypographicBounds(line, nil, nil, nil)))
    let contentWidth = fixed + textWidth
    var x = style.position.isLeft ? margin : frame.width - margin - contentWidth
    let dot = CGRect(x: x, y: plate.midY - dotSize / 2, width: dotSize, height: dotSize)
    x += dotSize + gap
    let logoRect = CGRect(
      x: x, y: plate.midY - logo.height / 2, width: logo.width, height: logo.height)
    x += logo.width + gap * 0.8
    let text = CGRect(x: x, y: plate.minY, width: textWidth, height: height)
    return WatermarkLayout(
      plate: plate, dot: dot, logo: logoRect, text: text, glowWidth: nil, line: line)
  }

  /// The wordmark `height` tall, or the person's image fitted to `height` and `maxWidth`.
  private static func logoSize(_ style: WatermarkStyle, height: CGFloat, maxWidth: CGFloat)
    -> CGSize
  {
    let aspect: CGFloat
    if let image = style.image, image.height > 0 {
      aspect = CGFloat(image.width) / CGFloat(image.height)
    } else {
      aspect = AnarlogBrand.logoSize.width / AnarlogBrand.logoSize.height
    }
    let width = min(height * aspect, maxWidth)
    return CGSize(width: width, height: width / aspect)
  }

  private static func corner(
    _ size: CGSize, in frame: CGSize, position: WatermarkStyle.Position, inset: CGFloat
  ) -> CGPoint {
    CGPoint(
      x: position.isLeft ? inset : frame.width - inset - size.width,
      y: position.isTop ? frame.height - inset - size.height : inset)
  }
}

/// Draws Anarlog's marks on camera frames. Meeting apps mirror only your own preview, so others
/// read what is drawn here as is: nothing is ever flipped.
public enum Watermark {
  public static let idleText = "Open Anarlog to use this camera"

  static let plateColor = CGColor(srgbRed: 0.11, green: 0.098, blue: 0.09, alpha: 0.74)

  /// The logo's and text's colour: white, or dark on a light glow tag.
  static func ink(_ style: WatermarkStyle) -> CGColor {
    style.layout == .glow && style.colorIsLight
      ? CGColor(srgbRed: 0.11, green: 0.098, blue: 0.09, alpha: 1) : white(1)
  }

  /// The overlay `style` describes, on a frame of `frame` size, at `time` seconds into its loop
  /// (`WatermarkMotion`). Time 0 is the still frame.
  public static func draw(
    _ style: WatermarkStyle, in ctx: CGContext, frame: CGSize, time: Double = 0
  ) {
    let layout = WatermarkLayout.make(style, frame: frame)
    ctx.saveGState()
    defer { ctx.restoreGState() }
    let faded = style.opacity < 1
    if faded {
      ctx.setAlpha(style.opacity)
      ctx.beginTransparencyLayer(auxiliaryInfo: nil)
    }
    switch style.layout {
    case .badge, .banner:
      drawPlate(layout, style: style, in: ctx, fill: plateColor, ink: ink(style), time: time)
    case .glow:
      drawGlow(
        style, width: layout.glowWidth ?? 0, strength: WatermarkMotion.glowStrength(at: time),
        in: ctx,
        frame: frame)
      drawPlate(layout, style: style, in: ctx, fill: style.cgColor, ink: ink(style), time: time)
    case .mark:
      ctx.setShadow(offset: .zero, blur: frame.height * 0.012, color: CGColor(gray: 0, alpha: 0.45))
      drawMark(style, in: ctx, rect: layout.logo, time: time)
    }
    if faded { ctx.endTransparencyLayer() }
  }

  /// What the mark's colour sweeps to: the badge red over a light mark, white over a dark one.
  static func markAccent(_ style: WatermarkStyle) -> CGColor {
    style.colorIsLight ? CGColor(srgbRed: 0.941, green: 0.271, blue: 0.271, alpha: 1) : white(1)
  }

  /// The mark layout: Anarlog's wordmark filled with its colour sweeping to the accent and back, or
  /// the person's image breathing.
  private static func drawMark(
    _ style: WatermarkStyle, in ctx: CGContext, rect: CGRect, time: Double
  ) {
    if style.image != nil {
      ctx.setAlpha(CGFloat(WatermarkMotion.imageAlpha(at: time)))
      drawLogo(style, in: ctx, rect: rect, color: style.cgColor)
      return
    }
    let stops = 12
    let samples = (0...stops).map {
      WatermarkMotion.markAccent(at: time, along: Double($0) / Double(stops))
    }
    guard samples.contains(where: { $0 > 0.001 }) else {
      drawLogo(style, in: ctx, rect: rect, color: style.cgColor)
      return
    }
    // Inside a transparency layer the shadow applies once, to the filled wordmark as a whole.
    ctx.beginTransparencyLayer(auxiliaryInfo: nil)
    drawLogo(style, in: ctx, rect: rect, color: style.cgColor)
    let base = style.cgColor.components ?? [1, 1, 1, 1]
    let accent = markAccent(style).components ?? [1, 1, 1, 1]
    let colors = samples.map { amount -> CGColor in
      let a = CGFloat(amount)
      return CGColor(
        srgbRed: base[0] + (accent[0] - base[0]) * a, green: base[1] + (accent[1] - base[1]) * a,
        blue: base[2] + (accent[2] - base[2]) * a, alpha: 1)
    }
    let locations = (0...stops).map { CGFloat($0) / CGFloat(stops) }
    if let gradient = CGGradient(
      colorsSpace: CGColorSpace(name: CGColorSpace.sRGB), colors: colors as CFArray,
      locations: locations)
    {
      ctx.setBlendMode(.sourceIn)
      ctx.drawLinearGradient(
        gradient, start: CGPoint(x: rect.minX, y: rect.midY),
        end: CGPoint(x: rect.maxX, y: rect.midY),
        options: [])
    }
    ctx.endTransparencyLayer()
  }

  private static func drawPlate(
    _ layout: WatermarkLayout, style: WatermarkStyle, in ctx: CGContext, fill: CGColor,
    ink: CGColor,
    time: Double
  ) {
    if let plate = layout.plate {
      ctx.setFillColor(fill)
      if style.layout == .banner {
        ctx.fill(plate)
        // A thin accent line along the band's inner edge.
        let line = max(1, (plate.height * 0.035).rounded())
        ctx.setFillColor(style.cgColor)
        ctx.fill(
          CGRect(
            x: plate.minX, y: style.position.isTop ? plate.minY : plate.maxY - line,
            width: plate.width, height: line))
      } else {
        let radius = plate.height / 2
        ctx.addPath(
          CGPath(roundedRect: plate, cornerWidth: radius, cornerHeight: radius, transform: nil))
        ctx.fillPath()
      }
    }
    if let dot = layout.dot {
      let alpha = CGFloat(WatermarkMotion.dotAlpha(at: time))
      ctx.setFillColor(style.cgColor.copy(alpha: alpha) ?? style.cgColor)
      ctx.fillEllipse(in: dot)
    }
    drawLogo(style, in: ctx, rect: layout.logo, color: ink)
    if let line = layout.line, let text = layout.text {
      drawLine(line, in: ctx, x: text.minX, centerY: text.midY)
    }
  }

  /// A soft band of `style.color` along every edge, strongest at the edge.
  private static func drawGlow(
    _ style: WatermarkStyle, width: CGFloat, strength: Double, in ctx: CGContext, frame: CGSize
  ) {
    guard width > 0 else { return }
    let steps = 24
    let step = width / CGFloat(steps)
    ctx.saveGState()
    ctx.setLineWidth(step + 0.5)
    for i in 0..<steps {
      let t = CGFloat(i) / CGFloat(steps)
      let alpha = 0.85 * CGFloat(strength) * pow(1 - t, 2.2)
      ctx.setStrokeColor(style.cgColor.copy(alpha: alpha) ?? style.cgColor)
      let inset = step * (CGFloat(i) + 0.5)
      ctx.stroke(CGRect(origin: .zero, size: frame).insetBy(dx: inset, dy: inset))
    }
    ctx.restoreGState()
  }

  private static func drawLogo(
    _ style: WatermarkStyle, in ctx: CGContext, rect: CGRect, color: CGColor
  ) {
    if let image = style.image {
      ctx.interpolationQuality = .high
      ctx.draw(image, in: rect)
    } else {
      AnarlogBrand.drawLogo(
        in: ctx, origin: CGPoint(x: rect.minX, y: rect.maxY), height: rect.height, color: color)
    }
  }

  /// Where the idle frame's wordmark sits (y up): centred, a little above the middle.
  public static func idleLogoRect(frame: CGSize) -> CGRect {
    let height = frame.height * 0.09
    let width = height * AnarlogBrand.logoSize.width / AnarlogBrand.logoSize.height
    return CGRect(
      x: (frame.width - width) / 2, y: frame.height / 2 - height * 0.15, width: width,
      height: height)
  }

  /// What the camera shows while Anarlog isn't feeding it: the camera is off in Anarlog, Anarlog
  /// Camera may not read the camera, or it is still opening the real camera.
  public static func drawIdle(in ctx: CGContext, frame: CGSize) {
    let unit = frame.height
    ctx.saveGState()
    ctx.setFillColor(CGColor(srgbRed: 0.11, green: 0.098, blue: 0.09, alpha: 1))
    ctx.fill(CGRect(origin: .zero, size: frame))

    let logo = idleLogoRect(frame: frame)
    AnarlogBrand.drawLogo(
      in: ctx, origin: CGPoint(x: logo.minX, y: logo.maxY), height: logo.height, color: white(1))

    let line = textLine(idleText, size: unit * 0.035, color: white(0.6), maxWidth: frame.width)
    let width = CGFloat(CTLineGetTypographicBounds(line, nil, nil, nil))
    drawLine(line, in: ctx, x: (frame.width - width) / 2, centerY: logo.minY - unit * 0.06)
    ctx.restoreGState()
  }

  /// Runs `body` with a CoreGraphics context drawing straight into a 32BGRA buffer.
  public static func withContext(on buffer: CVPixelBuffer, _ body: (CGContext, CGSize) -> Void) {
    CVPixelBufferLockBaseAddress(buffer, [])
    defer { CVPixelBufferUnlockBaseAddress(buffer, []) }
    let width = CVPixelBufferGetWidth(buffer)
    let height = CVPixelBufferGetHeight(buffer)
    guard
      let ctx = CGContext(
        data: CVPixelBufferGetBaseAddress(buffer), width: width, height: height,
        bitsPerComponent: 8, bytesPerRow: CVPixelBufferGetBytesPerRow(buffer),
        space: CGColorSpace(name: CGColorSpace.sRGB)!,
        bitmapInfo: CGImageAlphaInfo.premultipliedFirst.rawValue
          | CGBitmapInfo.byteOrder32Little.rawValue)
    else { return }
    body(ctx, CGSize(width: width, height: height))
  }

  /// A 32BGRA buffer of the frame size, for the idle frame and for tests.
  public static func makeBuffer(
    width: Int = CameraIdentity.width, height: Int = CameraIdentity.height
  )
    -> CVPixelBuffer?
  {
    var buffer: CVPixelBuffer?
    let attributes: [CFString: Any] = [
      kCVPixelBufferIOSurfacePropertiesKey: [:] as CFDictionary,
      kCVPixelBufferCGImageCompatibilityKey: true,
      kCVPixelBufferCGBitmapContextCompatibilityKey: true,
    ]
    CVPixelBufferCreate(
      kCFAllocatorDefault, width, height, kCVPixelFormatType_32BGRA, attributes as CFDictionary,
      &buffer)
    return buffer
  }

  static func white(_ alpha: CGFloat) -> CGColor {
    CGColor(srgbRed: 1, green: 1, blue: 1, alpha: alpha)
  }

  /// One line of the system font, cut with an ellipsis to `maxWidth`.
  static func textLine(_ text: String, size: CGFloat, color: CGColor, maxWidth: CGFloat) -> CTLine {
    let font =
      CTFontCreateUIFontForLanguage(.emphasizedSystem, size, nil)
      ?? CTFontCreateWithName("Helvetica-Bold" as CFString, size, nil)
    let attributes: [NSAttributedString.Key: Any] = [
      NSAttributedString.Key(kCTFontAttributeName as String): font,
      NSAttributedString.Key(kCTForegroundColorAttributeName as String): color,
    ]
    let line = CTLineCreateWithAttributedString(
      NSAttributedString(string: text, attributes: attributes))
    let ellipsis = CTLineCreateWithAttributedString(
      NSAttributedString(string: "…", attributes: attributes))
    return CTLineCreateTruncatedLine(line, Double(max(0, maxWidth)), .end, ellipsis) ?? line
  }

  static func drawLine(_ line: CTLine, in ctx: CGContext, x: CGFloat, centerY: CGFloat) {
    var ascent: CGFloat = 0
    var descent: CGFloat = 0
    CTLineGetTypographicBounds(line, &ascent, &descent, nil)
    ctx.textMatrix = .identity
    ctx.textPosition = CGPoint(x: x, y: centerY - (ascent - descent) / 2)
    CTLineDraw(line, ctx)
  }
}
