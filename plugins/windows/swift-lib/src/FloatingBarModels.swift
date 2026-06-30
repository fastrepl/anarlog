import Foundation

enum FloatingBarStatus: String, Codable {
  case recording
  case error
}

enum FloatingBarColorScheme: String, Codable {
  case light
  case dark
}

struct FloatingTranscriptBubblePayload: Codable, Identifiable {
  let id: String
  let speakerLabel: String
  let text: String
  let isSelf: Bool
  let isFinal: Bool
}

struct FloatingBarStatePayload: Codable {
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
  let transcriptBubbles: [FloatingTranscriptBubblePayload]
}
