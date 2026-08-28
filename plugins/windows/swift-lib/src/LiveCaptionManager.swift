import Foundation

final class LiveCaptionManager {
  static let shared = LiveCaptionManager()

  private init() {}

  func show() {
    hide(clearText: false)
  }

  func hide(clearText: Bool = true) {
    _ = clearText
    runOnMain {
      FloatingOverlaySettingsPanelManager.shared.hide()
    }
  }

  func update(state: LiveCaptionStatePayload) {
    _ = state
    hide(clearText: false)
  }

  private func runOnMain(_ block: @escaping () -> Void) {
    if Thread.isMainThread {
      block()
      return
    }

    DispatchQueue.main.sync(execute: block)
  }
}
