import CoreImage
import CoreVideo
import Foundation

/// Turns a camera frame into an Anarlog Camera frame: scaled to fill 1280x720, cropped to its
/// middle, the person's background replaced when they chose one, and `overlay` drawn on top when
/// there is one, so nothing behind the person can hide it. The live helper and
/// `anarlog-camera render` both go through here.
public final class Compositor {
  /// Vision's quality level for finding the person. Fast's 256x192 mask stair-steps and haloes
  /// at 1280x720; balanced's 512x384 holds a clean edge, and run every other frame (the mask
  /// reused and smoothed in between) it costs about what fast did every frame (README).
  public static let segmentation = "balanced"

  private let context = CIContext(options: [.cacheIntermediates: false])
  private var pool: CVPixelBufferPool?
  private let segmenter: PersonSegmenter
  /// The last image background, filled to the frame once rather than for every frame.
  private var backdrop: (key: WatermarkBackground, image: CIImage)?

  /// Vision finds the person every this many frames; the mask is reused in between.
  public static let segmentationInterval = 2

  /// `segmentation` is `fast`, `balanced` or `accurate` (Vision's quality levels); unknown is the
  /// default. Vision runs every `segmentationInterval` frames.
  public init(
    segmentation: String = Compositor.segmentation,
    segmentationInterval: Int = Compositor.segmentationInterval
  ) {
    segmenter = PersonSegmenter(
      quality: PersonSegmenter.Quality(rawValue: segmentation) ?? .balanced,
      interval: segmentationInterval)
    let attributes: [CFString: Any] = [
      kCVPixelBufferPixelFormatTypeKey: kCVPixelFormatType_32BGRA,
      kCVPixelBufferWidthKey: CameraIdentity.width,
      kCVPixelBufferHeightKey: CameraIdentity.height,
      kCVPixelBufferIOSurfacePropertiesKey: [:] as CFDictionary,
    ]
    CVPixelBufferPoolCreate(kCFAllocatorDefault, nil, attributes as CFDictionary, &pool)
  }

  /// `time` is seconds into the overlay's loop (`WatermarkMotion`): the live camera passes each
  /// frame's timestamp; 0 is the still frame.
  public func compose(
    _ image: CIImage, overlay: WatermarkStyle?, background: WatermarkBackground = .none,
    time: Double = 0
  ) -> CVPixelBuffer? {
    guard let output = makeBuffer() else { return nil }
    let target = CGRect(x: 0, y: 0, width: CameraIdentity.width, height: CameraIdentity.height)
    let space = CGColorSpace(name: CGColorSpace.sRGB)
    let filled = Self.fill(image, target.size)
    if background == .none {
      context.render(filled, to: output, bounds: target, colorSpace: space)
    } else {
      // Vision reads the frame as the camera sends it; the blend then writes the output.
      guard let frame = makeBuffer() else { return nil }
      context.render(filled, to: frame, bounds: target, colorSpace: space)
      let camera = CIImage(cvPixelBuffer: frame)
      // Nobody found yet: the background alone, never the room the person asked to hide.
      let mask = segmenter.mask(frame)
      let behind = self.behind(camera, background, mask: mask)
      let composed =
        mask.map {
          camera.applyingFilter(
            "CIBlendWithMask",
            parameters: [kCIInputBackgroundImageKey: behind, kCIInputMaskImageKey: $0])
        } ?? behind
      context.render(composed, to: output, bounds: target, colorSpace: space)
    }
    if let overlay {
      Watermark.withContext(on: output) { ctx, size in
        Watermark.draw(overlay, in: ctx, frame: size, time: time)
      }
    }
    return output
  }

  private func makeBuffer() -> CVPixelBuffer? {
    var buffer: CVPixelBuffer?
    if let pool { CVPixelBufferPoolCreatePixelBuffer(kCFAllocatorDefault, pool, &buffer) }
    return buffer
  }

  /// How strongly Blur softens the room: a share of the frame's height.
  static let blurRadius: Double = 0.03

  /// What goes behind the person in `camera`, at its size; `mask` is where the person is.
  private func behind(_ camera: CIImage, _ background: WatermarkBackground, mask: CIImage?)
    -> CIImage
  {
    let extent = camera.extent
    switch background {
    case .none:
      return camera
    case .blur:
      let sigma = extent.height * Self.blurRadius
      guard let mask else {
        return camera.clampedToExtent().applyingGaussianBlur(sigma: sigma).cropped(to: extent)
      }
      // The room alone, blurred: the person is cut out first (transparent), and dividing by the
      // blurred coverage fills their place from the room around them, so no dark halo of hair
      // spreads into the blur.
      let room = CIImage(color: .clear).cropped(to: extent).applyingFilter(
        "CIBlendWithMask",
        parameters: [kCIInputBackgroundImageKey: camera, kCIInputMaskImageKey: mask])
      return room.clampedToExtent().applyingGaussianBlur(sigma: sigma).cropped(to: extent)
        .unpremultiplyingAlpha().settingAlphaOne(in: extent)
    case .scene(let scene):
      return Self.fill(BackgroundScene.image(scene), extent.size)
    case .image(let image):
      if let backdrop, backdrop.key == background { return backdrop.image }
      let filled = Self.fill(CIImage(cgImage: image), extent.size)
      // Drawn to pixels once, so each frame only blends it.
      let image =
        context.createCGImage(filled, from: filled.extent).map { CIImage(cgImage: $0) } ?? filled
      backdrop = (background, image)
      return image
    }
  }

  /// Scales `image` to cover `size` and centres it, cropping what overflows.
  static func fill(_ image: CIImage, _ size: CGSize) -> CIImage {
    let extent = image.extent
    let scale = max(size.width / extent.width, size.height / extent.height)
    let scaled = image.transformed(by: CGAffineTransform(scaleX: scale, y: scale))
    let dx = (scaled.extent.width - size.width) / 2 + scaled.extent.minX
    let dy = (scaled.extent.height - size.height) / 2 + scaled.extent.minY
    return scaled.transformed(by: CGAffineTransform(translationX: -dx, y: -dy))
      .cropped(to: CGRect(origin: .zero, size: size))
  }

  /// `frame` as a PNG, scaled down to `size` first when given: a meeting app shrinks the
  /// camera's 1280x720 the same way for a small tile.
  public func png(_ frame: CVPixelBuffer, size: CGSize? = nil) -> Data? {
    var image = CIImage(cvPixelBuffer: frame)
    if let size, size.width > 0, size.height > 0 { image = Self.scaled(image, to: size) }
    guard let space = CGColorSpace(name: CGColorSpace.sRGB) else { return nil }
    return context.pngRepresentation(of: image, format: .RGBA8, colorSpace: space)
  }

  /// `image` scaled down to fit `size` the way a meeting app shrinks the camera for a tile.
  static func scaled(_ image: CIImage, to size: CGSize) -> CIImage {
    let scale = min(size.width / image.extent.width, size.height / image.extent.height)
    guard scale != 1 else { return image }
    return image.applyingFilter(
      "CILanczosScaleTransform", parameters: [kCIInputScaleKey: scale, kCIInputAspectRatioKey: 1])
  }
}
