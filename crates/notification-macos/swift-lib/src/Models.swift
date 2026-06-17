import Foundation

struct Participant: Codable {
  let name: String?
  let email: String
  let status: String
}

struct EventDetails: Codable {
  let what: String
  let timezone: String?
  let location: String?
}

struct NotificationFooter: Codable {
  let text: String
  let actionLabel: String
  let icon: NotificationIcon?
}

enum NotificationIconAsset: Codable {
  case appIcon
  case calendar
  case systemSymbol(name: String)
  case bundleId(bundleId: String)
  case path(path: String)

  private enum CodingKeys: String, CodingKey {
    case type
    case name
    case bundleId = "bundle_id"
    case path
  }

  private enum IconType: String, Codable {
    case appIcon = "app_icon"
    case calendar
    case systemSymbol = "system_symbol"
    case bundleId = "bundle_id"
    case path
  }

  init(from decoder: Decoder) throws {
    let container = try decoder.container(keyedBy: CodingKeys.self)

    switch try container.decode(IconType.self, forKey: .type) {
    case .appIcon:
      self = .appIcon
    case .calendar:
      self = .calendar
    case .systemSymbol:
      self = .systemSymbol(name: try container.decode(String.self, forKey: .name))
    case .bundleId:
      self = .bundleId(bundleId: try container.decode(String.self, forKey: .bundleId))
    case .path:
      self = .path(path: try container.decode(String.self, forKey: .path))
    }
  }

  func encode(to encoder: Encoder) throws {
    var container = encoder.container(keyedBy: CodingKeys.self)

    switch self {
    case .appIcon:
      try container.encode(IconType.appIcon, forKey: .type)
    case .calendar:
      try container.encode(IconType.calendar, forKey: .type)
    case .systemSymbol(let name):
      try container.encode(IconType.systemSymbol, forKey: .type)
      try container.encode(name, forKey: .name)
    case .bundleId(let bundleId):
      try container.encode(IconType.bundleId, forKey: .type)
      try container.encode(bundleId, forKey: .bundleId)
    case .path(let path):
      try container.encode(IconType.path, forKey: .type)
      try container.encode(path, forKey: .path)
    }
  }
}

enum NotificationIcon: Codable {
  case hidden
  case bundleId(bundleId: String)
  case systemSymbol(name: String)
  case path(path: String)
  case overlay(base: NotificationIconAsset, badge: NotificationIconAsset)

  private enum CodingKeys: String, CodingKey {
    case type
    case name
    case bundleId = "bundle_id"
    case path
    case base
    case badge
  }

  private enum IconType: String, Codable {
    case hidden
    case bundleId = "bundle_id"
    case systemSymbol = "system_symbol"
    case path
    case overlay
  }

  init(from decoder: Decoder) throws {
    let container = try decoder.container(keyedBy: CodingKeys.self)

    switch try container.decode(IconType.self, forKey: .type) {
    case .hidden:
      self = .hidden
    case .bundleId:
      self = .bundleId(bundleId: try container.decode(String.self, forKey: .bundleId))
    case .systemSymbol:
      self = .systemSymbol(name: try container.decode(String.self, forKey: .name))
    case .path:
      self = .path(path: try container.decode(String.self, forKey: .path))
    case .overlay:
      self = .overlay(
        base: try container.decode(NotificationIconAsset.self, forKey: .base),
        badge: try container.decode(NotificationIconAsset.self, forKey: .badge)
      )
    }
  }

  func encode(to encoder: Encoder) throws {
    var container = encoder.container(keyedBy: CodingKeys.self)

    switch self {
    case .hidden:
      try container.encode(IconType.hidden, forKey: .type)
    case .bundleId(let bundleId):
      try container.encode(IconType.bundleId, forKey: .type)
      try container.encode(bundleId, forKey: .bundleId)
    case .systemSymbol(let name):
      try container.encode(IconType.systemSymbol, forKey: .type)
      try container.encode(name, forKey: .name)
    case .path(let path):
      try container.encode(IconType.path, forKey: .type)
      try container.encode(path, forKey: .path)
    case .overlay(let base, let badge):
      try container.encode(IconType.overlay, forKey: .type)
      try container.encode(base, forKey: .base)
      try container.encode(badge, forKey: .badge)
    }
  }
}

enum NotificationSource: Codable {
  case calendarEvent(eventId: String)
  case session(sessionId: String)
  case micDetected(appNames: [String], appIds: [String], eventIds: [String])

  private enum CodingKeys: String, CodingKey {
    case type
    case eventId = "event_id"
    case sessionId = "session_id"
    case appNames = "app_names"
    case appIds = "app_ids"
    case eventIds = "event_ids"
  }

  private enum SourceType: String, Codable {
    case calendarEvent = "calendar_event"
    case session = "session"
    case micDetected = "mic_detected"
  }

  init(from decoder: Decoder) throws {
    let container = try decoder.container(keyedBy: CodingKeys.self)

    switch try container.decode(SourceType.self, forKey: .type) {
    case .calendarEvent:
      self = .calendarEvent(eventId: try container.decode(String.self, forKey: .eventId))
    case .session:
      self = .session(sessionId: try container.decode(String.self, forKey: .sessionId))
    case .micDetected:
      self = .micDetected(
        appNames: try container.decode([String].self, forKey: .appNames),
        appIds: try container.decodeIfPresent([String].self, forKey: .appIds) ?? [],
        eventIds: try container.decodeIfPresent([String].self, forKey: .eventIds) ?? []
      )
    }
  }

  func encode(to encoder: Encoder) throws {
    var container = encoder.container(keyedBy: CodingKeys.self)

    switch self {
    case .calendarEvent(let eventId):
      try container.encode(SourceType.calendarEvent, forKey: .type)
      try container.encode(eventId, forKey: .eventId)
    case .session(let sessionId):
      try container.encode(SourceType.session, forKey: .type)
      try container.encode(sessionId, forKey: .sessionId)
    case .micDetected(let appNames, let appIds, let eventIds):
      try container.encode(SourceType.micDetected, forKey: .type)
      try container.encode(appNames, forKey: .appNames)
      try container.encode(appIds, forKey: .appIds)
      try container.encode(eventIds, forKey: .eventIds)
    }
  }

}

enum NotificationActionVariant: String, Codable {
  case defaultAction = "default"
  case destructive = "destructive"
}

struct NotificationPayload: Codable {
  let key: String
  let title: String
  let message: String
  let timeoutSeconds: Double
  let source: NotificationSource?
  let startTime: Int64?
  let participants: [Participant]?
  let eventDetails: EventDetails?
  let actionLabel: String?
  let actionVariant: NotificationActionVariant?
  let options: [String]?
  let footer: NotificationFooter?
  let icon: NotificationIcon?

  var isPersistent: Bool {
    return timeoutSeconds <= 0
  }

  var isDestructiveAction: Bool {
    return actionVariant == .destructive
  }

  var showsStopCountdown: Bool {
    return isDestructiveAction && actionLabel == "Stop" && timeoutSeconds > 0
  }

  var hasOptions: Bool {
    guard let options = options else { return false }
    return !options.isEmpty
  }

  var hasExpandableContent: Bool {
    if case .some(.calendarEvent) = source {
      return false
    }

    let hasParticipants = participants?.isEmpty == false
    return hasParticipants || eventDetails != nil
  }
}
