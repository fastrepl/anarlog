import Foundation

enum FloatingBarStatus: String, Codable, Equatable {
  case recording
  case reconnecting
  case error
}

enum FloatingBarColorScheme: String, Codable, Equatable {
  case light
  case dark
}

struct FloatingTranscriptBubblePayload: Codable, Identifiable, Equatable {
  let id: String
  let speakerLabel: String
  let text: String
  let isSelf: Bool
  let isFinal: Bool
  let startMs: Double
  let endMs: Double
  let overlapsPrevious: Bool
  let overlapsNext: Bool
}

struct FloatingDictationPayload: Codable, Equatable {
  let sessionId: String
  let phase: String
  let microphone: String
  let text: String
  let partial: String
  let previewEnabled: Bool
  let previewUnavailable: Bool
}

struct FloatingBarStatePayload: Codable {
  let dictation: FloatingDictationPayload?
  let amplitude: Double
  let title: String
  let status: FloatingBarStatus
  let colorScheme: FloatingBarColorScheme
  let opacity: Double
  let liveCaptionOpacity: Double
  let liveCaptionWidth: Double
  let liveCaptionLineCount: Int
  let liveCaptionPosition: LiveCaptionPosition
  let liveCaptionMinimized: Bool
  let liveCaptionToggleVisible: Bool
  let transcriptBubbles: [FloatingTranscriptBubblePayload]?
}
