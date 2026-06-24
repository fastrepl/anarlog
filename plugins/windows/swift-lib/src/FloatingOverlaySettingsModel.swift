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

  private var pendingLiveCaptionPosition: LiveCaptionPosition?
  private var pendingLiveCaptionMinimized: Bool?

  private init() {}

  func apply(floatingBarState state: FloatingBarStatePayload) {
    floatingBarOpacity = clampedOpacity(state.opacity)
    liveCaptionOpacity = clampedOpacity(state.liveCaptionOpacity)
    _ = applyLiveCaptionPosition(state.liveCaptionPosition)
    _ = applyLiveCaptionMinimized(state.liveCaptionMinimized)
  }

  func apply(liveCaptionState state: LiveCaptionStatePayload) -> Bool {
    liveCaptionOpacity = clampedOpacity(state.opacity)
    let positionChanged = applyLiveCaptionPosition(state.position)
    let minimizedChanged = applyLiveCaptionMinimized(state.minimized)
    return positionChanged || minimizedChanged
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
    pendingLiveCaptionPosition = value
    RustBridge.floatingBarSettingsChanged(
      FloatingOverlaySettingsChangePayload(liveCaptionPosition: value))
  }

  func setLiveCaptionMinimized(_ value: Bool) {
    guard liveCaptionMinimized != value else { return }
    liveCaptionMinimized = value
    pendingLiveCaptionMinimized = value
    RustBridge.floatingBarSettingsChanged(
      FloatingOverlaySettingsChangePayload(liveCaptionMinimized: value))
  }

  private func applyLiveCaptionPosition(_ value: LiveCaptionPosition) -> Bool {
    if let pendingLiveCaptionPosition {
      guard pendingLiveCaptionPosition == value else { return false }
      self.pendingLiveCaptionPosition = nil
    }

    guard liveCaptionPosition != value else { return false }
    liveCaptionPosition = value
    return true
  }

  private func applyLiveCaptionMinimized(_ value: Bool) -> Bool {
    if let pendingLiveCaptionMinimized {
      guard pendingLiveCaptionMinimized == value else { return false }
      self.pendingLiveCaptionMinimized = nil
    }

    guard liveCaptionMinimized != value else { return false }
    liveCaptionMinimized = value
    return true
  }

  private func clampedOpacity(_ value: Double) -> Double {
    min(max(value, 0.35), 0.95)
  }
}
