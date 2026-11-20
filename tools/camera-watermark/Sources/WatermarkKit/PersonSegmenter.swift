import CoreImage
import CoreVideo
import Vision

/// Finds the person in camera frames with Vision, so Anarlog Camera can put its own background behind
/// them. One per frame sequence: Vision and the smoothing here both carry the outline from frame
/// to frame.
///
/// Vision's mask is small (256x192 at `fast`, 512x384 at `balanced`, 4:3 for a 16:9 frame) and
/// hard-edged, so drawn as is it stair-steps and flickers, and its edge takes in a sliver of the
/// room, a light halo against a darker background. So the mask is:
/// - asked of Vision every `interval` frames and reused in between;
/// - blended with the last one (`persistence`), so the edge holds still instead of shimmering
///   from frame to frame and a stray shape that shows for one frame stays under the ramp below;
///   kept at the mask's own size, so it costs next to nothing;
/// - scaled up to the frame with linear sampling and softened, then cut along a ramp that sits a
///   little inside Vision's edge (no halo) and spans a few pixels (a smooth, anti-aliased edge).
final class PersonSegmenter {
  enum Quality: String, CaseIterable, Sendable {
    case fast, balanced, accurate

    var level: VNGeneratePersonSegmentationRequest.QualityLevel {
      switch self {
      case .fast: return .fast
      case .balanced: return .balanced
      case .accurate: return .accurate
      }
    }
  }

  /// How much of the last mask each new one keeps: 0 is none, so the outline follows at once.
  static let persistence: CGFloat = 0.45
  /// The edge ramp, in mask values: below `low` is room, above `high` the person. Above 0.5, so
  /// the edge sits just inside Vision's, where its own blur had spilled onto the room.
  static let low: CGFloat = 0.45
  static let high: CGFloat = 0.85

  private let request = VNGeneratePersonSegmentationRequest()
  private let sequence = VNSequenceRequestHandler()
  private let interval: Int
  private let context = CIContext(options: [
    .cacheIntermediates: false, .workingColorSpace: NSNull(),
  ])
  /// The smoothed mask at Vision's size, in two buffers that take turns being read and written.
  private var smoothed: [CVPixelBuffer] = []
  private var current = 0
  private var hasMask = false
  private var frame = 0

  init(quality: Quality, interval: Int = 1) {
    self.interval = max(1, interval)
    request.qualityLevel = quality.level
    request.outputPixelFormat = kCVPixelFormatType_OneComponent8
  }

  /// Where the person is in `frame`, white on black, at the frame's size; nil until Vision has
  /// found someone once.
  func mask(_ frame: CVPixelBuffer) -> CIImage? {
    defer { self.frame += 1 }
    if self.frame % interval == 0 || !hasMask { update(frame) }
    guard hasMask else { return nil }
    let extent = CGRect(
      x: 0, y: 0, width: CVPixelBufferGetWidth(frame), height: CVPixelBufferGetHeight(frame))
    let small = CIImage(cvPixelBuffer: smoothed[current])
    let scaled = small.samplingLinear().transformed(
      by: CGAffineTransform(
        scaleX: extent.width / small.extent.width, y: extent.height / small.extent.height))
    // Soften by about a source pixel, so the ramp below draws a smooth curve, not its steps.
    let soft = scaled.clampedToExtent()
      .applyingGaussianBlur(sigma: Double(extent.width / small.extent.width) * 0.6)
      .cropped(to: extent)
    return Self.ramp(soft)
  }

  private func update(_ frame: CVPixelBuffer) {
    guard (try? sequence.perform([request], on: frame)) != nil,
      let found = request.results?.first?.pixelBuffer
    else { return }
    let width = CVPixelBufferGetWidth(found)
    let height = CVPixelBufferGetHeight(found)
    if smoothed.count != 2 || CVPixelBufferGetWidth(smoothed[0]) != width
      || CVPixelBufferGetHeight(smoothed[0]) != height
    {
      smoothed = (0..<2).compactMap { _ in Self.buffer(width: width, height: height) }
      hasMask = false
      guard smoothed.count == 2 else { return }
    }
    let new = CIImage(cvPixelBuffer: found)
    let next = 1 - current
    let blended =
      hasMask
      ? new.applyingFilter(
        "CIDissolveTransition",
        parameters: [
          kCIInputTargetImageKey: CIImage(cvPixelBuffer: smoothed[current]),
          kCIInputTimeKey: Self.persistence,
        ])
      : new
    context.render(blended, to: smoothed[next], bounds: new.extent, colorSpace: nil)
    current = next
    hasMask = true
  }

  /// `(v - low) / (high - low)`, clamped to 0...1, on the mask's one channel.
  private static func ramp(_ mask: CIImage) -> CIImage {
    let scale = 1 / (high - low)
    let gain = CIVector(x: scale, y: 0, z: 0, w: 0)
    return mask.applyingFilter(
      "CIColorMatrix",
      parameters: [
        "inputRVector": gain, "inputGVector": CIVector(x: 0, y: scale, z: 0, w: 0),
        "inputBVector": CIVector(x: 0, y: 0, z: scale, w: 0),
        "inputBiasVector": CIVector(x: -low * scale, y: -low * scale, z: -low * scale, w: 1),
      ]
    ).applyingFilter("CIColorClamp")
  }

  private static func buffer(width: Int, height: Int) -> CVPixelBuffer? {
    var buffer: CVPixelBuffer?
    CVPixelBufferCreate(
      kCFAllocatorDefault, width, height, kCVPixelFormatType_OneComponent8,
      [kCVPixelBufferIOSurfacePropertiesKey: [:]] as CFDictionary, &buffer)
    return buffer
  }
}
