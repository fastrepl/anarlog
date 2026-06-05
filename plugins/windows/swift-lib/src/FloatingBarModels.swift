import Foundation

enum FloatingBarStatus: String, Codable {
  case recording
  case error
}

struct FloatingBarStatePayload: Codable {
  let amplitude: Double
  let status: FloatingBarStatus
}
