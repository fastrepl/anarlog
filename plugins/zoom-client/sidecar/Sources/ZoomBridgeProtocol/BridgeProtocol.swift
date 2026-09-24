import Foundation

// Wire mirror of `crates/meeting-capture/src/adapter.rs`. Keep field names and enum
// spellings in sync with the Rust serde attributes; the Rust side is authoritative.

public let bridgeProtocolVersion: UInt16 = 1

public enum MeetingPlatform: String, Codable {
  case googleMeet = "google_meet"
  case zoom
  case microsoftTeams = "microsoft_teams"
  case webex
  case jitsi
}

public enum CaptureProviderKind: String, Codable {
  case anarlog
  case recall
  case zoomRtms = "zoom_rtms"
  case microsoftGraph = "microsoft_graph"
  case webexMeetingsSdk = "webex_meetings_sdk"
  case zoomMeetingSdk = "zoom_meeting_sdk"
}

public enum BotState: String, Codable {
  case queued, launching
  case waitingForAdmission = "waiting_for_admission"
  case joined, capturing, stopping, completed, failed, canceled
}

public enum TerminalReasonKind: String, Codable {
  case meetingEnded = "meeting_ended"
  case stoppedByRequest = "stopped_by_request"
  case admissionDenied = "admission_denied"
  case admissionTimeout = "admission_timeout"
  case noOneJoined = "no_one_joined"
  case everyoneLeft = "everyone_left"
  case removedFromMeeting = "removed_from_meeting"
  case recordingPermissionDenied = "recording_permission_denied"
  case invalidMeeting = "invalid_meeting"
  case authenticationFailed = "authentication_failed"
  case capacityExceeded = "capacity_exceeded"
  case networkLost = "network_lost"
  case providerError = "provider_error"
  case workerExited = "worker_exited"
  case unknown
}

public struct TerminalReason: Codable, Equatable {
  public var kind: TerminalReasonKind
  public var message: String?
  public var retryable: Bool

  public init(kind: TerminalReasonKind, message: String? = nil, retryable: Bool = false) {
    self.kind = kind
    self.message = message
    self.retryable = retryable
  }
}

public struct MeetingReference: Codable, Equatable {
  public var platform: MeetingPlatform
  public var url: String
  public var externalId: String?
  public var calendarEventId: String?

  enum CodingKeys: String, CodingKey {
    case platform, url
    case externalId = "external_id"
    case calendarEventId = "calendar_event_id"
  }
}

public struct CaptureWorkerCheckpoint: Codable, Equatable {
  public var jobId: String
  public var botId: String
  public var provider: CaptureProviderKind
  public var meeting: MeetingReference
  public var state: BotState
  public var nextSequence: UInt64

  enum CodingKeys: String, CodingKey {
    case jobId = "job_id"
    case botId = "bot_id"
    case provider, meeting, state
    case nextSequence = "next_sequence"
  }
}

public struct BridgeStart: Codable, Equatable {
  public var protocolVersion: UInt16
  public var checkpoint: CaptureWorkerCheckpoint
  public var botName: String
}

public enum BridgeCommand: Equatable {
  case start(BridgeStart)
  case stop(jobId: String)
}

extension BridgeCommand: Codable {
  private enum CodingKeys: String, CodingKey { case type, data }
  private enum StopKeys: String, CodingKey { case jobId = "job_id" }

  public init(from decoder: Decoder) throws {
    let container = try decoder.container(keyedBy: CodingKeys.self)
    switch try container.decode(String.self, forKey: .type) {
    case "start":
      self = .start(try container.decode(BridgeStart.self, forKey: .data))
    case "stop":
      let data = try container.nestedContainer(keyedBy: StopKeys.self, forKey: .data)
      self = .stop(jobId: try data.decode(String.self, forKey: .jobId))
    case let other:
      throw DecodingError.dataCorruptedError(
        forKey: .type, in: container, debugDescription: "unknown command \(other)")
    }
  }

  public func encode(to encoder: Encoder) throws {
    var container = encoder.container(keyedBy: CodingKeys.self)
    switch self {
    case .start(let start):
      try container.encode("start", forKey: .type)
      try container.encode(start, forKey: .data)
    case .stop(let jobId):
      try container.encode("stop", forKey: .type)
      var data = container.nestedContainer(keyedBy: StopKeys.self, forKey: .data)
      try data.encode(jobId, forKey: .jobId)
    }
  }
}

public struct Participant: Codable, Equatable {
  public var id: String
  public var displayName: String?
  public var email: String?

  public init(id: String, displayName: String? = nil, email: String? = nil) {
    self.id = id
    self.displayName = displayName
    self.email = email
  }

  enum CodingKeys: String, CodingKey {
    case id
    case displayName = "display_name"
    case email
  }
}

public struct ActiveSpeakers: Codable, Equatable {
  public var atMs: UInt64
  public var participantIds: [String]

  public init(atMs: UInt64, participantIds: [String]) {
    self.atMs = atMs
    self.participantIds = participantIds
  }

  enum CodingKeys: String, CodingKey {
    case atMs = "at_ms"
    case participantIds = "participant_ids"
  }
}

public struct BridgeTerminal: Codable, Equatable {
  public var state: BotState
  public var reason: TerminalReason

  public init(state: BotState, reason: TerminalReason) {
    self.state = state
    self.reason = reason
  }
}

public enum BridgeEventPayload: Equatable {
  case ready
  case waitingForAdmission
  case joined
  case capturing
  case participantUpserted(Participant)
  case participantLeft(participantId: String)
  case activeSpeakers(ActiveSpeakers)
  case terminal(BridgeTerminal)
}

extension BridgeEventPayload: Codable {
  private enum CodingKeys: String, CodingKey { case type, data }
  private enum LeftKeys: String, CodingKey { case participantId = "participant_id" }

  public init(from decoder: Decoder) throws {
    let container = try decoder.container(keyedBy: CodingKeys.self)
    switch try container.decode(String.self, forKey: .type) {
    case "ready": self = .ready
    case "waiting_for_admission": self = .waitingForAdmission
    case "joined": self = .joined
    case "capturing": self = .capturing
    case "participant_upserted":
      self = .participantUpserted(try container.decode(Participant.self, forKey: .data))
    case "participant_left":
      let data = try container.nestedContainer(keyedBy: LeftKeys.self, forKey: .data)
      self = .participantLeft(participantId: try data.decode(String.self, forKey: .participantId))
    case "active_speakers":
      self = .activeSpeakers(try container.decode(ActiveSpeakers.self, forKey: .data))
    case "terminal":
      self = .terminal(try container.decode(BridgeTerminal.self, forKey: .data))
    case let other:
      throw DecodingError.dataCorruptedError(
        forKey: .type, in: container, debugDescription: "unknown event \(other)")
    }
  }

  public func encode(to encoder: Encoder) throws {
    var container = encoder.container(keyedBy: CodingKeys.self)
    switch self {
    case .ready: try container.encode("ready", forKey: .type)
    case .waitingForAdmission: try container.encode("waiting_for_admission", forKey: .type)
    case .joined: try container.encode("joined", forKey: .type)
    case .capturing: try container.encode("capturing", forKey: .type)
    case .participantUpserted(let participant):
      try container.encode("participant_upserted", forKey: .type)
      try container.encode(participant, forKey: .data)
    case .participantLeft(let participantId):
      try container.encode("participant_left", forKey: .type)
      var data = container.nestedContainer(keyedBy: LeftKeys.self, forKey: .data)
      try data.encode(participantId, forKey: .participantId)
    case .activeSpeakers(let speakers):
      try container.encode("active_speakers", forKey: .type)
      try container.encode(speakers, forKey: .data)
    case .terminal(let terminal):
      try container.encode("terminal", forKey: .type)
      try container.encode(terminal, forKey: .data)
    }
  }
}

public struct BridgeEvent: Codable, Equatable {
  public var protocolVersion: UInt16
  public var sequence: UInt64
  public var platform: MeetingPlatform
  public var provider: CaptureProviderKind
  public var payload: BridgeEventPayload
}

/// Stamps outgoing events with the session identity and a bridge sequence that starts at
/// zero for every process, which `MeetingSdkBridgeNormalizer` on the Rust side enforces.
public final class BridgeEventStream {
  private let platform: MeetingPlatform
  private let provider: CaptureProviderKind
  private var nextSequence: UInt64 = 0
  private let write: (Data) -> Void
  private let encoder: JSONEncoder = {
    let encoder = JSONEncoder()
    encoder.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
    return encoder
  }()

  public init(checkpoint: CaptureWorkerCheckpoint, write: @escaping (Data) -> Void) {
    self.platform = checkpoint.meeting.platform
    self.provider = checkpoint.provider
    self.write = write
  }

  @discardableResult
  public func emit(_ payload: BridgeEventPayload) -> BridgeEvent {
    let event = BridgeEvent(
      protocolVersion: bridgeProtocolVersion,
      sequence: nextSequence,
      platform: platform,
      provider: provider,
      payload: payload)
    nextSequence += 1
    var line = try! encoder.encode(event)
    line.append(0x0A)
    write(line)
    return event
  }
}

public enum ZoomMeetingURL {
  /// Extracts (meetingNumber, passcode) from a `https://*.zoom.us/j/<id>?pwd=<pwd>` style link.
  public static func parse(_ raw: String) -> (meetingNumber: Int64, passcode: String?)? {
    guard let url = URL(string: raw),
      let components = URLComponents(url: url, resolvingAgainstBaseURL: false)
    else { return nil }
    let parts = url.path.split(separator: "/").map(String.init)
    guard let index = parts.firstIndex(where: { $0 == "j" || $0 == "w" || $0 == "s" }),
      parts.indices.contains(index + 1),
      let number = Int64(parts[index + 1].filter(\.isNumber)), number > 0
    else { return nil }
    let passcode = components.queryItems?.first(where: { $0.name == "pwd" })?.value
    return (number, passcode?.isEmpty == false ? passcode : nil)
  }
}
