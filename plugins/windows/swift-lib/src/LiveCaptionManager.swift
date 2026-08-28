import Foundation

final class LiveCaptionManager {
  static let shared = LiveCaptionManager()

  private init() {}

  func show() {}

  func hide(clearText: Bool = true) {
    _ = clearText
  }

  func update(state: LiveCaptionStatePayload) {
    _ = state
  }
}
