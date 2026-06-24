import Foundation

enum FloatingBarStatus: String, Codable {
  case recording
  case error
}

enum FloatingBarColorScheme: String, Codable {
  case light
  case dark
}

struct FloatingBarStatePayload: Codable {
  let amplitude: Double
  let status: FloatingBarStatus
  let colorScheme: FloatingBarColorScheme
  let opacity: Double
  let liveCaptionOpacity: Double
  let liveCaptionWidth: Double
  let liveCaptionLineCount: Int
  let liveCaptionPosition: LiveCaptionPosition
  let liveCaptionMinimized: Bool
  let liveCaptionToggleVisible: Bool
}
