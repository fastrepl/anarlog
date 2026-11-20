import Foundation
import ServiceManagement
import WatermarkKit

/// Anarlog Camera's login agent (`anarlog-camera agent`), registered with launchd from this app's
/// `Contents/Library/LaunchAgents`. Anarlog turns it on with the camera and off without it.
enum AgentService {
  private static var service: SMAppService { SMAppService.agent(plistName: AgentFiles.plistName) }

  /// Registers the agent, or restarts it when it runs an older Anarlog Camera than this one.
  static func enable() -> (state: String, detail: String?) {
    let service = service
    let ours = Bundle.main.object(forInfoDictionaryKey: "CFBundleVersion") as? String
    if service.status == .enabled {
      if runningVersion() == ours { return (name(service.status), nil) }
      // launchd keeps an updated agent's old process: unregistering stops it.
      try? service.unregister()
    }
    do {
      try service.register()
      return (name(service.status), nil)
    } catch {
      return (name(service.status), "\(error)")
    }
  }

  static func disable() -> (state: String, detail: String?) {
    let service = service
    guard service.status == .enabled || service.status == .requiresApproval else {
      return (name(service.status), nil)
    }
    do {
      try service.unregister()
      return (name(service.status), nil)
    } catch {
      return (name(service.status), "\(error)")
    }
  }

  /// The version the running agent last reported, if it reported lately.
  private static func runningVersion() -> String? {
    guard
      let attributes = try? FileManager.default.attributesOfItem(atPath: AgentFiles.status.path),
      let modified = attributes[.modificationDate] as? Date,
      Date().timeIntervalSince(modified) < 10,
      let data = try? Data(contentsOf: AgentFiles.status),
      let object = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any]
    else { return nil }
    return object["version"] as? String
  }

  private static func name(_ status: SMAppService.Status) -> String {
    switch status {
    case .enabled: return "enabled"
    case .requiresApproval: return "requires_approval"
    case .notFound: return "not_found"
    default: return "not_registered"
    }
  }
}
