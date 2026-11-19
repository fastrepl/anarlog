import AVFoundation
import CoreImage
import Foundation
import WatermarkKit

// anarlog-camera: Anarlog Camera.app's executable.
//   anarlog-camera agent   Anarlog Camera's login agent (launchd): while the camera is on in
//                          Anarlog, shows the person's real camera whenever an app uses Anarlog
//                          Camera, with the background behind them and the badge while Anarlog
//                          records. Reads and writes `AgentFiles`.
//   anarlog-camera serve   (default) JSON lines on stdin/stdout with Anarlog: the extension and the
//                          agent's registration. Never opens the camera.
//   anarlog-camera render --output out.png [--input photo.jpg | --camera [--frames 300]]
//                      [--preset badge|glow|mark|banner] [--style '{"layout":"badge",...}']
//                      [--background '{"kind":"blur"}'] [--segmentation fast|balanced|accurate]
//                      [--segmentation-every N] [--tile 640x360] [--time 1.2] [--idle] [--no-badge]
//                       writes one Anarlog Camera frame (at --time seconds into its motion) over
//                       --input, or over a plain grey frame, without the extension. --camera reads
//                       the real camera instead, runs --frames frames through the live path,
//                       writes the last, and reports time and CPU per frame on stderr.

let arguments = Array(CommandLine.arguments.dropFirst())
switch arguments.first {
case "render":
  exit(Render.run(Array(arguments.dropFirst())))
case "agent":
  Agent.shared.run()
case nil, "serve":
  Serve.shared.run()
default:
  FileHandle.standardError.write(
    Data("usage: anarlog-camera [serve | agent | render --output out.png]\n".utf8))
  exit(64)
}

/// Anarlog's side: the Camera Extension and the agent's registration. It never opens the camera;
/// only the agent does.
final class Serve {
  static let shared = Serve()

  func run() {
    emit("ready")
    Thread.detachNewThread {
      while let line = readLine() {
        guard let command = HelperCommand.parse(line) else {
          self.emit("error", ["message": "unknown_command"])
          continue
        }
        DispatchQueue.main.async { self.handle(command) }
      }
      // Anarlog went away. The agent keeps the camera going on its own.
      exit(0)
    }
    RunLoop.main.run()
  }

  private func handle(_ command: HelperCommand) {
    switch command {
    case .activate:
      ExtensionRequest.submit(removing: false) { state, detail in
        self.reportExtension(state, detail)
      }
    case .deactivate:
      ExtensionRequest.submit(removing: true) { state, detail in self.reportExtension(state, detail)
      }
    case .enableAgent:
      reportAgent(AgentService.enable())
    case .disableAgent:
      reportAgent(AgentService.disable())
    case .check:
      // Whether macOS publishes the camera: the extension is in, whatever this session asked.
      emit("camera", ["installed": SinkClient().refreshDevice()])
    }
  }

  private func reportExtension(_ state: ExtensionState, _ detail: String?) {
    var fields: [String: Any] = ["state": state.rawValue]
    if let detail { fields["detail"] = detail }
    emit("extension", fields)
  }

  private func reportAgent(_ result: (state: String, detail: String?)) {
    var fields: [String: Any] = ["state": result.state]
    if let detail = result.detail { fields["detail"] = detail }
    emit("agent", fields)
  }

  private func emit(_ event: String, _ fields: [String: Any] = [:]) {
    print(HelperEvent.line(event, fields))
    fflush(stdout)
  }
}

/// Anarlog Camera's login agent. Twice a second it reads what Anarlog asks (`state.json`) and who
/// uses Anarlog Camera. While an app does and the camera is on in Anarlog, it opens the real
/// camera and feeds the extension; when the last one leaves, it lets the camera go, so the camera
/// light is off otherwise. It writes what it sees to `status.json` when that changes, and at least
/// every few seconds so Anarlog can tell it's alive.
final class Agent {
  static let shared = Agent()

  private let sink = SinkClient()
  private lazy var feed = CameraFeed(sink: sink) { [weak self] in
    self?.look ?? FrameLook()
  }
  /// The background, and the style while the badge shows. Read on the capture queue for every
  /// frame.
  private var look: FrameLook {
    get { lookLock.withLock { _look } }
    set { lookLock.withLock { _look = newValue } }
  }
  private var _look = FrameLook()
  private let lookLock = NSLock()

  private var state = AgentState()
  private var stateModified: Date?
  private var feedState = FeedState()
  private var feedProblem: String?
  private var askedCamera = false
  private var lastCameraRequest = 0.0
  private var lastStatus = ""
  private var lastStatusWrite = Date.distantPast
  private let version = Bundle.main.object(forInfoDictionaryKey: "CFBundleVersion") as? String ?? ""

  /// How often the agent looks, and how often it rewrites an unchanged status as a heartbeat.
  static let tick: TimeInterval = 0.5
  static let heartbeat: TimeInterval = 2

  func run() {
    Timer.scheduledTimer(withTimeInterval: Self.tick, repeats: true) { [weak self] _ in self?.step()
    }
    step()
    RunLoop.main.run()
  }

  private func step() {
    readState()
    if sink.refreshDevice(), state.enabled {
      feedState.consumers = sink.consumers()
    } else {
      feedState.consumers = []
    }
    feedState.recording = state.records { kill($0, 0) == 0 || errno == EPERM }
    askForCameraIfNeeded()
    look = FrameLook(
      background: state.background, backgroundID: state.backgroundID,
      overlay: feedState.showsBadge ? state.style : nil)
    if feedState.capturesCamera {
      if !sink.isStarted, !sink.start() { feedProblem = "sink_unavailable" }
      if sink.isStarted, !feed.isRunning { feedProblem = feed.start() }
      if feed.isRunning { feedProblem = nil }
    } else {
      feed.stop()
      sink.stop()
      feedProblem = nil
    }
    writeStatus()
  }

  private func readState() {
    let modified =
      (try? FileManager.default.attributesOfItem(atPath: AgentFiles.state.path))?[
        .modificationDate] as? Date
    guard modified != stateModified else { return }
    stateModified = modified
    state = AgentState.decode(try? Data(contentsOf: AgentFiles.state))
  }

  /// The agent reads the camera under its own permission: macOS asks the person once, when an
  /// app first shows Anarlog Camera or when Anarlog's "Allow camera" asks.
  private func askForCameraIfNeeded() {
    guard state.enabled, AVCaptureDevice.authorizationStatus(for: .video) == .notDetermined else {
      return
    }
    let requested = state.cameraRequest > lastCameraRequest
    lastCameraRequest = state.cameraRequest
    guard requested || (!feedState.consumers.isEmpty && !askedCamera) else { return }
    askedCamera = true
    AVCaptureDevice.requestAccess(for: .video) { _ in
      DispatchQueue.main.async { self.step() }
    }
  }

  private func writeStatus() {
    var fields: [String: Any] = [
      "camera_installed": sink.deviceID != nil,
      "consumers": feedState.consumers,
      "recording": feedState.recording,
      "badge": feedState.showsBadge,
      "camera_permission": Self.permission(),
      // What each new frame puts behind the person; `step` set `look` before this. Frames
      // already queued may still show the last one: only `protected_background_id` says the
      // room is hidden in what meeting apps get.
      "background": look.background.kind,
      "version": version,
    ]
    if let name = feed.cameraName { fields["source_camera"] = name }
    if let feedProblem { fields["problem"] = feedProblem }
    if feedState.capturesCamera, feed.isRunning, sink.hidesRoom(for: state.backgroundID) {
      fields["protected_background_id"] = state.backgroundID
    }
    let line = HelperEvent.line("status", fields)
    let now = Date()
    guard line != lastStatus || now.timeIntervalSince(lastStatusWrite) >= Self.heartbeat else {
      return
    }
    lastStatus = line
    lastStatusWrite = now
    AgentFiles.write(Data(line.utf8), to: AgentFiles.status)
  }

  private static func permission() -> String {
    switch AVCaptureDevice.authorizationStatus(for: .video) {
    case .authorized: return "allowed"
    case .denied, .restricted: return "denied"
    default: return "not_asked"
    }
  }
}

/// The harness: one frame through the same compositor the live camera uses.
enum Render {
  static func run(_ args: [String]) -> Int32 {
    func value(_ flag: String) -> String? {
      args.firstIndex(of: flag).flatMap { args.indices.contains($0 + 1) ? args[$0 + 1] : nil }
    }
    guard let output = value("--output") else {
      FileHandle.standardError.write(Data("render needs --output\n".utf8))
      return 64
    }
    var style = WatermarkStyle.badge
    if let name = value("--preset") {
      guard let preset = WatermarkStyle.preset(name) else {
        FileHandle.standardError.write(Data("unknown preset \(name)\n".utf8))
        return 64
      }
      style = preset
    }
    if let json = value("--style") {
      guard let data = json.data(using: .utf8),
        let decoded = WatermarkStyle.decode(try? JSONSerialization.jsonObject(with: data))
      else {
        FileHandle.standardError.write(Data("--style needs a JSON object\n".utf8))
        return 64
      }
      style = decoded
    }
    var background = WatermarkBackground.none
    if let json = value("--background") {
      guard let data = json.data(using: .utf8),
        let decoded = WatermarkBackground.decode(try? JSONSerialization.jsonObject(with: data))
      else {
        FileHandle.standardError.write(Data("--background needs a JSON object\n".utf8))
        return 64
      }
      background = decoded
    }
    let tile = value("--tile").flatMap(size)
    let time = value("--time").flatMap(Double.init) ?? 0
    let overlay = args.contains("--no-badge") ? nil : style

    let compositor = Compositor(
      segmentation: value("--segmentation") ?? Compositor.segmentation,
      segmentationInterval: value("--segmentation-every").flatMap(Int.init)
        ?? Compositor.segmentationInterval)
    if args.contains("--camera") {
      let count = value("--frames").flatMap(Int.init) ?? 300
      guard
        let frame = CameraBench.run(
          compositor, frames: max(1, count), overlay: overlay, background: background)
      else { return 1 }
      guard let png = compositor.png(frame, size: tile),
        (try? png.write(to: URL(fileURLWithPath: output))) != nil
      else {
        FileHandle.standardError.write(Data("couldn't render \(output)\n".utf8))
        return 1
      }
      print(output)
      return 0
    }
    let input: CIImage
    if let path = value("--input") {
      guard let image = CIImage(contentsOf: URL(fileURLWithPath: path)) else {
        FileHandle.standardError.write(Data("couldn't read \(path)\n".utf8))
        return 64
      }
      input = image
    } else {
      input = CIImage(color: CIColor(red: 0.5, green: 0.5, blue: 0.5))
        .cropped(to: CGRect(x: 0, y: 0, width: CameraIdentity.width, height: CameraIdentity.height))
    }
    let frame: CVPixelBuffer?
    if args.contains("--idle") {
      frame = Watermark.makeBuffer()
      if let frame { Watermark.withContext(on: frame) { Watermark.drawIdle(in: $0, frame: $1) } }
    } else {
      frame = compositor.compose(input, overlay: overlay, background: background, time: time)
    }
    guard let frame, let png = compositor.png(frame, size: tile),
      (try? png.write(to: URL(fileURLWithPath: output))) != nil
    else {
      FileHandle.standardError.write(Data("couldn't render \(output)\n".utf8))
      return 1
    }
    print(output)
    return 0
  }

  /// `640x360` as a size.
  private static func size(_ text: String) -> CGSize? {
    let parts = text.split(separator: "x").compactMap { Double($0) }
    guard parts.count == 2, parts[0] > 0, parts[1] > 0 else { return nil }
    return CGSize(width: parts[0], height: parts[1])
  }
}
