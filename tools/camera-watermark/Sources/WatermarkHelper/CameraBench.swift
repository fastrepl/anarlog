import AVFoundation
import CoreImage
import Darwin
import WatermarkKit

/// `render --camera`: the real camera through the live path's compositor, timed. Each frame is
/// composed on the capture queue as `CameraFeed` does, and the time and the process's CPU over
/// the run are reported on stderr.
enum CameraBench {
  static func run(
    _ compositor: Compositor, frames count: Int, overlay: WatermarkStyle?,
    background: WatermarkBackground
  ) -> CVPixelBuffer? {
    guard AVCaptureDevice.authorizationStatus(for: .video) == .authorized,
      let camera = CameraFeed.pickCamera()
    else {
      FileHandle.standardError.write(Data("no camera this process may read\n".utf8))
      return nil
    }
    let reader = Reader(compositor, count: count, overlay: overlay, background: background)
    let session = AVCaptureSession()
    session.beginConfiguration()
    if session.canSetSessionPreset(.hd1280x720) { session.sessionPreset = .hd1280x720 }
    guard let input = try? AVCaptureDeviceInput(device: camera), session.canAddInput(input) else {
      return nil
    }
    session.addInput(input)
    let output = AVCaptureVideoDataOutput()
    output.videoSettings = [kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA]
    output.alwaysDiscardsLateVideoFrames = true
    output.setSampleBufferDelegate(reader, queue: reader.queue)
    session.addOutput(output)
    session.commitConfiguration()
    session.startRunning()
    reader.done.wait()
    session.stopRunning()
    reader.report(camera.localizedName)
    return reader.last
  }

  private final class Reader: NSObject, AVCaptureVideoDataOutputSampleBufferDelegate {
    let queue = DispatchQueue(label: "so.anarlog.camera.bench")
    let done = DispatchSemaphore(value: 0)
    private let compositor: Compositor
    private let count: Int
    private let overlay: WatermarkStyle?
    private let background: WatermarkBackground
    /// The camera's first frames adjust exposure; they're composed but not counted.
    private let warmUp = 30
    private var seen = 0
    private var times: [Double] = []
    private var cpuStart = 0.0
    private var wallStart = 0.0
    private var cpuEnd = 0.0
    private var wallEnd = 0.0
    var last: CVPixelBuffer?

    init(
      _ compositor: Compositor, count: Int, overlay: WatermarkStyle?,
      background: WatermarkBackground
    ) {
      self.compositor = compositor
      self.count = count
      self.overlay = overlay
      self.background = background
    }

    func captureOutput(
      _ output: AVCaptureOutput, didOutput sampleBuffer: CMSampleBuffer,
      from connection: AVCaptureConnection
    ) {
      guard seen < warmUp + count, let pixels = CMSampleBufferGetImageBuffer(sampleBuffer) else {
        return
      }
      if seen == warmUp {
        cpuStart = Self.cpuSeconds()
        wallStart = Self.now()
      }
      let start = Self.now()
      let time = CMSampleBufferGetPresentationTimeStamp(sampleBuffer).seconds
      last = compositor.compose(
        CIImage(cvPixelBuffer: pixels), overlay: overlay, background: background,
        time: time.isFinite ? time : 0)
      if seen >= warmUp { times.append(Self.now() - start) }
      seen += 1
      if seen == warmUp + count {
        cpuEnd = Self.cpuSeconds()
        wallEnd = Self.now()
        done.signal()
      }
    }

    func report(_ camera: String) {
      let sorted = times.sorted()
      guard !sorted.isEmpty else { return }
      let mean = sorted.reduce(0, +) / Double(sorted.count)
      let p95 = sorted[min(sorted.count - 1, Int(Double(sorted.count) * 0.95))]
      let wall = wallEnd - wallStart
      let line = String(
        format:
          "camera=%@ frames=%d fps=%.1f compose_ms_mean=%.2f compose_ms_p95=%.2f compose_ms_max=%.2f cpu_percent=%.1f\n",
        camera, sorted.count, Double(sorted.count) / wall, mean * 1000, p95 * 1000,
        sorted.last! * 1000, (cpuEnd - cpuStart) / wall * 100)
      FileHandle.standardError.write(Data(line.utf8))
    }

    private static func now() -> Double { Double(DispatchTime.now().uptimeNanoseconds) / 1e9 }

    /// User and system CPU time of this process so far, in seconds.
    private static func cpuSeconds() -> Double {
      var usage = rusage()
      getrusage(RUSAGE_SELF, &usage)
      func seconds(_ t: timeval) -> Double { Double(t.tv_sec) + Double(t.tv_usec) / 1e6 }
      return seconds(usage.ru_utime) + seconds(usage.ru_stime)
    }
  }
}
