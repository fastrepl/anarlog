import Foundation

enum Phase: String, Codable {
  case recording
  case processing
}

struct DictationStatePayload: Codable {
  let phase: Phase
  let amplitude: Float
}
