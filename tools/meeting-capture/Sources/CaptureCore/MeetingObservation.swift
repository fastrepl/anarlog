import Foundation

/// UI observations are hints about one capture, not verified platform identities.
public struct MeetingParticipantObservation: Codable, Equatable {
  public let id: String
  public let displayName: String
  /// Nil when the UI does not explicitly identify this participant as local or remote.
  public let isLocal: Bool?

  public init(id: String, displayName: String, isLocal: Bool?) {
    self.id = id
    self.displayName = displayName
    self.isLocal = isLocal
  }
}

public struct MeetingObservation: Codable, Equatable {
  public let platform: String
  public let scopeID: String
  public let participants: [MeetingParticipantObservation]
  /// Only visible roster rows are observed; this is never a complete membership snapshot.
  public let rosterVisible: Bool
  /// How many people the app itself says are in the call, the person included, when its roster
  /// shows one count for everyone. Nil when the app shows none, or more than one group.
  public let participantCount: Int?
  public let speakers: [SpeakerCue]
  public let speakerStatus: String
  public let source: String

  public init(
    platform: String, scopeID: String, participants: [MeetingParticipantObservation],
    speakers: [SpeakerCue], speakerStatus: String, source: String = "accessibility",
    rosterVisible: Bool = false, participantCount: Int? = nil
  ) {
    self.platform = platform
    self.scopeID = scopeID
    self.participants = participants
    self.rosterVisible = rosterVisible
    self.participantCount = participantCount
    self.speakers = speakers
    self.speakerStatus = speakerStatus
    self.source = source
  }
}

/// An explicit speaking signal from a platform adapter. Accessibility has shown none so far, so
/// every observation carries an empty list and `speakerStatus` `unavailable`.
public struct SpeakerCue: Codable, Equatable {
  public let platform: String
  public let participantID: String
  public let displayName: String

  public init(platform: String, participantID: String, displayName: String) {
    self.platform = platform
    self.participantID = participantID
    self.displayName = displayName
  }
}
