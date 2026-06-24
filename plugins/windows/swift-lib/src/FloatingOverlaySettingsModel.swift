import Combine
import Foundation

struct FloatingOverlaySettingsChangePayload: Codable {
  var floatingBarOpacity: Double?
  var liveCaptionOpacity: Double?
  var liveCaptionPosition: LiveCaptionPosition?
  var liveCaptionMinimized: Bool?
}

final class FloatingOverlaySettingsModel: ObservableObject {
  static let shared = FloatingOverlaySettingsModel()

  @Published var floatingBarOpacity: Double = 0.78
  @Published var liveCaptionOpacity: Double = 0.78
  @Published var liveCaptionPosition: LiveCaptionPosition = .topCenter
  @Published var liveCaptionMinimized: Bool = false

  private init() {}

  func apply(floatingBarState state: FloatingBarStatePayload) {
    floatingBarOpacity = clampedOpacity(state.opacity)
    liveCaptionOpacity = clampedOpacity(state.liveCaptionOpacity)
    liveCaptionPosition = state.liveCaptionPosition
    liveCaptionMinimized = state.liveCaptionMinimized
  }

  func apply(liveCaptionState state: LiveCaptionStatePayload) {
    liveCaptionOpacity = clampedOpacity(state.opacity)
    liveCaptionPosition = state.position
    liveCaptionMinimized = state.minimized
  }

  func setFloatingBarOpacity(_ value: Double) {
    let nextValue = clampedOpacity(value)
    guard floatingBarOpacity != nextValue else { return }
    floatingBarOpacity = nextValue
    RustBridge.floatingBarSettingsChanged(
      FloatingOverlaySettingsChangePayload(floatingBarOpacity: nextValue))
  }

  func setLiveCaptionOpacity(_ value: Double) {
    let nextValue = clampedOpacity(value)
    guard liveCaptionOpacity != nextValue else { return }
    liveCaptionOpacity = nextValue
    RustBridge.floatingBarSettingsChanged(
      FloatingOverlaySettingsChangePayload(liveCaptionOpacity: nextValue))
  }

  func setLiveCaptionPosition(_ value: LiveCaptionPosition) {
    guard liveCaptionPosition != value else { return }
    liveCaptionPosition = value
    RustBridge.floatingBarSettingsChanged(
      FloatingOverlaySettingsChangePayload(liveCaptionPosition: value))
  }

  func setLiveCaptionMinimized(_ value: Bool) {
    guard liveCaptionMinimized != value else { return }
    liveCaptionMinimized = value
    RustBridge.floatingBarSettingsChanged(
      FloatingOverlaySettingsChangePayload(liveCaptionMinimized: value))
  }

  private func clampedOpacity(_ value: Double) -> Double {
    min(max(value, 0.35), 0.95)
  }
}
