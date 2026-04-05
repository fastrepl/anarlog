import Cocoa

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
}

enum NotificationIconAsset: Codable {
  case appIcon
  case calendar
  case bundleId(bundleId: String)
  case path(path: String)

  private enum CodingKeys: String, CodingKey {
    case type
    case bundleId = "bundle_id"
    case path
  }

  private enum IconType: String, Codable {
    case appIcon = "app_icon"
    case calendar
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
  case path(path: String)
  case overlay(base: NotificationIconAsset, badge: NotificationIconAsset)

  private enum CodingKeys: String, CodingKey {
    case type
    case bundleId = "bundle_id"
    case path
    case base
    case badge
  }

  private enum IconType: String, Codable {
    case hidden
    case bundleId = "bundle_id"
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
  case micDetected(appNames: [String], appIds: [String]?, eventIds: [String]?)

  private enum CodingKeys: String, CodingKey {
    case type
    case eventId = "event_id"
    case appNames = "app_names"
    case appIds = "app_ids"
    case eventIds = "event_ids"
  }

  private enum SourceType: String, Codable {
    case calendarEvent = "calendar_event"
    case micDetected = "mic_detected"
  }

  init(from decoder: Decoder) throws {
    let container = try decoder.container(keyedBy: CodingKeys.self)

    switch try container.decode(SourceType.self, forKey: .type) {
    case .calendarEvent:
      self = .calendarEvent(eventId: try container.decode(String.self, forKey: .eventId))
    case .micDetected:
      self = .micDetected(
        appNames: try container.decode([String].self, forKey: .appNames),
        appIds: try container.decodeIfPresent([String].self, forKey: .appIds),
        eventIds: try container.decodeIfPresent([String].self, forKey: .eventIds)
      )
    }
  }

  func encode(to encoder: Encoder) throws {
    var container = encoder.container(keyedBy: CodingKeys.self)

    switch self {
    case .calendarEvent(let eventId):
      try container.encode(SourceType.calendarEvent, forKey: .type)
      try container.encode(eventId, forKey: .eventId)
    case .micDetected(let appNames, let appIds, let eventIds):
      try container.encode(SourceType.micDetected, forKey: .type)
      try container.encode(appNames, forKey: .appNames)
      try container.encodeIfPresent(appIds, forKey: .appIds)
      try container.encodeIfPresent(eventIds, forKey: .eventIds)
    }
  }

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
  let options: [String]?
  let footer: NotificationFooter?
  let icon: NotificationIcon?

  var isPersistent: Bool {
    return timeoutSeconds <= 0
  }

  var hasOptions: Bool {
    guard let options = options else { return false }
    return !options.isEmpty
  }

  var hasExpandableContent: Bool {
    let hasParticipants = participants?.isEmpty == false
    return hasParticipants || eventDetails != nil
  }
}

enum ParticipantStatusDisplay {
  case accepted
  case maybe
  case declined

  init(from string: String) {
    switch string.lowercased() {
    case "accepted": self = .accepted
    case "maybe": self = .maybe
    case "declined": self = .declined
    default: self = .accepted
    }
  }

  var icon: String {
    switch self {
    case .accepted: return "✓"
    case .maybe: return "?"
    case .declined: return "✗"
    }
  }

  var color: NSColor {
    switch self {
    case .accepted: return NSColor.systemGreen
    case .maybe: return NSColor.systemYellow
    case .declined: return NSColor.systemRed
    }
  }
}
