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

struct NotificationPayload: Codable {
  let key: String
  let title: String
  let message: String
  let timeoutSeconds: Double
  let startTime: Int64?
  let participants: [Participant]?
  let eventDetails: EventDetails?
  let actionLabel: String?
  let options: [String]?

  var isPersistent: Bool {
    return timeoutSeconds <= 0
  }

  var hasOptions: Bool {
    guard let options = options else { return false }
    return !options.isEmpty
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
