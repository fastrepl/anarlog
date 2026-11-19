import AVFoundation
import CoreImage
import WatermarkKit

/// What one frame gets: the background behind the person, and the style while Anarlog records.
/// `backgroundID` is the Background request the frame is drawn for (`ProtectedOutput`).
struct FrameLook {
  var background: WatermarkBackground = .none
  var backgroundID = ""
  var overlay: WatermarkStyle?
}

/// Reads the person's real camera and writes Anarlog Camera frames to the sink.
final class CameraFeed: NSObject, AVCaptureVideoDataOutputSampleBufferDelegate {
  private let sink: SinkClient
  private let look: () -> FrameLook
  private let compositor = Compositor()
  private let queue = DispatchQueue(label: "so.anarlog.camera.capture")
  private var session: AVCaptureSession?
  private var observers: [NSObjectProtocol] = []
  private(set) var cameraName: String?

  /// `look` is read for every frame: what goes behind the person, and the style to draw on top.
  init(sink: SinkClient, look: @escaping () -> FrameLook) {
    self.sink = sink
    self.look = look
  }

  var isRunning: Bool { session != nil }

  /// The camera Anarlog Camera wraps: the one macOS prefers (the last one the person chose),
  /// else the built-in one. Never a virtual camera (`CameraIdentity.canWrap`). iPhones through
  /// Continuity Camera are left out, as they would be picked up whenever one comes near.
  static func pickCamera() -> AVCaptureDevice? {
    let candidates = AVCaptureDevice.DiscoverySession(
      deviceTypes: [.builtInWideAngleCamera, .external], mediaType: .video, position: .unspecified
    ).devices.filter {
      CameraIdentity.canWrap(deviceName: $0.localizedName, transportType: $0.transportType)
    }
    if let preferred = AVCaptureDevice.systemPreferredCamera,
      candidates.contains(where: { $0.uniqueID == preferred.uniqueID })
    {
      return preferred
    }
    return candidates.first { $0.deviceType == .builtInWideAngleCamera } ?? candidates.first
  }

  /// Starts reading the camera; returns why it can't.
  func start() -> String? {
    guard session == nil else { return nil }
    guard AVCaptureDevice.authorizationStatus(for: .video) == .authorized else {
      return "camera_not_allowed"
    }
    guard let camera = Self.pickCamera() else { return "no_camera" }
    let session = AVCaptureSession()
    session.beginConfiguration()
    if session.canSetSessionPreset(.hd1280x720) { session.sessionPreset = .hd1280x720 }
    guard let input = try? AVCaptureDeviceInput(device: camera), session.canAddInput(input) else {
      return "camera_unavailable"
    }
    session.addInput(input)
    let output = AVCaptureVideoDataOutput()
    output.videoSettings = [kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA]
    output.alwaysDiscardsLateVideoFrames = true
    output.setSampleBufferDelegate(self, queue: queue)
    guard session.canAddOutput(output) else { return "camera_unavailable" }
    session.addOutput(output)
    session.commitConfiguration()
    session.startRunning()
    self.session = session
    cameraName = camera.localizedName
    // A session that dies (the webcam unplugged, the camera taken) is dropped here and rebuilt
    // on the helper's next tick, with whichever camera is there then.
    let center = NotificationCenter.default
    observers = [
      center.addObserver(
        forName: AVCaptureSession.runtimeErrorNotification, object: session, queue: .main
      ) {
        [weak self, weak session] _ in
        if let session, self?.session === session { self?.stop() }
      },
      center.addObserver(
        forName: AVCaptureDevice.wasDisconnectedNotification, object: camera, queue: .main
      ) {
        [weak self, weak session] _ in
        if let session, self?.session === session { self?.stop() }
      },
    ]
    return nil
  }

  func stop() {
    observers.forEach(NotificationCenter.default.removeObserver)
    observers = []
    session?.stopRunning()
    session = nil
    cameraName = nil
  }

  func captureOutput(
    _ output: AVCaptureOutput, didOutput sampleBuffer: CMSampleBuffer,
    from connection: AVCaptureConnection
  ) {
    // The frame's own timestamp drives the watermark's motion, so it moves with the video.
    let time = CMSampleBufferGetPresentationTimeStamp(sampleBuffer).seconds
    let look = look()
    guard let pixels = CMSampleBufferGetImageBuffer(sampleBuffer),
      let frame = compositor.compose(
        CIImage(cvPixelBuffer: pixels), overlay: look.overlay, background: look.background,
        time: time.isFinite ? time : 0),
      let sample = Self.sample(frame, timing: sampleBuffer)
    else { return }
    sink.send(sample, backgroundID: look.backgroundID, hidesRoom: look.background != .none)
  }

  private static func sample(_ buffer: CVPixelBuffer, timing source: CMSampleBuffer)
    -> CMSampleBuffer?
  {
    var format: CMFormatDescription?
    CMVideoFormatDescriptionCreateForImageBuffer(
      allocator: kCFAllocatorDefault, imageBuffer: buffer, formatDescriptionOut: &format)
    guard let format else { return nil }
    var timing = CMSampleTimingInfo(
      duration: CMSampleBufferGetDuration(source),
      presentationTimeStamp: CMSampleBufferGetPresentationTimeStamp(source),
      decodeTimeStamp: .invalid)
    var sample: CMSampleBuffer?
    CMSampleBufferCreateReadyWithImageBuffer(
      allocator: kCFAllocatorDefault, imageBuffer: buffer, formatDescription: format,
      sampleTiming: &timing, sampleBufferOut: &sample)
    return sample
  }
}
