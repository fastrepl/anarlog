import Foundation

/// Anarlog Camera's agent and Anarlog talk through two small files in one folder, so the camera
/// keeps showing the person while Anarlog is closed or restarting:
///
/// - `state.json`, which Anarlog writes: whether the camera is on, whether this Mac records (and
///   Anarlog's pid, so a crash never leaves a recording claimed), the style, the background, and a
///   request to ask for camera access.
/// - `status.json`, which the agent writes: one `status` line.
///
/// The folder is Anarlog Camera's own, apart from Char Camera's.
public enum AgentFiles {
  /// The agent's launchd label, and its plist in `Anarlog Camera.app/Contents/Library/LaunchAgents`.
  public static let label = "so.anarlog.camera.agent"
  public static var plistName: String { "\(label).plist" }

  public static var directory: URL {
    FileManager.default.homeDirectoryForCurrentUser
      .appendingPathComponent("Library/Application Support/so.anarlog.camera", isDirectory: true)
  }
  public static var state: URL { directory.appendingPathComponent("state.json") }
  public static var status: URL { directory.appendingPathComponent("status.json") }

  /// Writes `data` whole: a reader never sees half a file.
  public static func write(_ data: Data, to url: URL) {
    try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    try? data.write(to: url, options: .atomic)
  }
}

/// What Anarlog asks of the agent (`state.json`). A missing or unreadable file is the camera off.
///
/// ```json
/// {"enabled":true,"recording":false,"owner_pid":4242,"style":{…},"background":{…},
///  "background_id":"…","camera_request":0}
/// ```
public struct AgentState: Equatable, Sendable {
  public var enabled: Bool
  /// Anarlog records a meeting. Only counts while `ownerPID` is alive.
  public var recording: Bool
  public var ownerPID: Int32
  public var style: WatermarkStyle
  public var background: WatermarkBackground
  /// New each time Anarlog asks for a Background: the agent reports `protected_background_id`
  /// only for frames drawn and sent for this request (`ProtectedOutput`). Empty names no request,
  /// and never gets that receipt.
  public var backgroundID: String
  /// Bumped by Anarlog's "Allow camera": the agent asks macOS for camera access once per value.
  public var cameraRequest: Double

  public init(
    enabled: Bool = false, recording: Bool = false, ownerPID: Int32 = 0,
    style: WatermarkStyle = .badge, background: WatermarkBackground = .none,
    backgroundID: String = "",
    cameraRequest: Double = 0
  ) {
    self.enabled = enabled
    self.recording = recording
    self.ownerPID = ownerPID
    self.style = style
    self.background = background
    self.backgroundID = backgroundID
    self.cameraRequest = cameraRequest
  }

  public static func decode(_ data: Data?) -> AgentState {
    guard let data, let object = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any]
    else { return AgentState() }
    return AgentState(
      enabled: object["enabled"] as? Bool ?? false,
      recording: object["recording"] as? Bool ?? false,
      ownerPID: (object["owner_pid"] as? NSNumber)?.int32Value ?? 0,
      style: WatermarkStyle.decode(object["style"]) ?? .badge,
      background: WatermarkBackground.decode(object["background"]) ?? .none,
      backgroundID: object["background_id"] as? String ?? "",
      cameraRequest: (object["camera_request"] as? NSNumber)?.doubleValue ?? 0)
  }

  /// Whether the badge may show: Anarlog says it records, and Anarlog is still running.
  public func records(isAlive: (Int32) -> Bool) -> Bool {
    enabled && recording && ownerPID > 0 && isAlive(ownerPID)
  }
}
