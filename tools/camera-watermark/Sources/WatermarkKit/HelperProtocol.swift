import Foundation

/// One JSON object per line on `anarlog-camera serve`'s stdin, from Anarlog. What the camera shows
/// (on, recording, style, background) goes to the agent in `AgentFiles.state` instead, which
/// Anarlog writes itself.
public enum HelperCommand: Equatable, Sendable {
  /// Submit the system extension for activation; macOS asks the person to allow it.
  case activate
  case deactivate
  /// Register Anarlog Camera's login agent with launchd, restarting an older one.
  case enableAgent
  case disableAgent
  /// Look for the Anarlog Camera device; answered by a `camera` event.
  case check

  public static func parse(_ line: String) -> HelperCommand? {
    guard let data = line.data(using: .utf8),
      let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
      let command = object["command"] as? String
    else { return nil }
    switch command {
    case "activate": return .activate
    case "deactivate": return .deactivate
    case "enable_agent": return .enableAgent
    case "disable_agent": return .disableAgent
    case "check": return .check
    default: return nil
    }
  }
}

/// The extension's state as `OSSystemExtensionRequest` reports it.
public enum ExtensionState: String, Sendable {
  case active
  case needsApproval = "needs_approval"
  case failed
  case removed
}

/// One JSON object per line on the helper's stdout.
public enum HelperEvent {
  public static func line(_ event: String, _ fields: [String: Any] = [:]) -> String {
    var object = fields
    object["event"] = event
    let data =
      (try? JSONSerialization.data(withJSONObject: object, options: [.sortedKeys])) ?? Data()
    return String(decoding: data, as: UTF8.self)
  }
}
