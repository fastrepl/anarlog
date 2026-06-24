import Combine
import Foundation

struct FloatingOverlaySettingsChangePayload: Codable {
  var floatingBarOpacity: Double?
  var liveCaptionOpacity: Double?
  var liveCaptionPosition: LiveCaptionPosition?
  var liveCaptionMinimized: Bool?
}

enum FloatingOverlayOpacity {
  static let minFloatingBar = 0.35
  static let minLiveCaption = 0.05
  static let maxFloatingBar = 0.95
  static let maxLiveCaption = 1.0
}

final class FloatingOverlaySettingsModel: ObservableObject {
  static let shared = FloatingOverlaySettingsModel()

  @Published var floatingBarOpacity: Double = 0.78
  @Published var liveCaptionOpacity: Double = 0.30
  @Published var liveCaptionPosition: LiveCaptionPosition = .topCenter
  @Published var liveCaptionMinimized: Bool = false

  private var pendingFloatingBarOpacity: Double?
  private var pendingLiveCaptionOpacity: Double?
  private var pendingLiveCaptionPosition: LiveCaptionPosition?
  private var pendingLiveCaptionMinimized: Bool?

  private init() {}

  func apply(floatingBarState state: FloatingBarStatePayload) {
    applyFloatingBarOpacity(state.opacity)
    applyLiveCaptionOpacity(state.liveCaptionOpacity)
    _ = applyLiveCaptionPosition(state.liveCaptionPosition)
    _ = applyLiveCaptionMinimized(state.liveCaptionMinimized)
  }

  func apply(liveCaptionState state: LiveCaptionStatePayload) -> Bool {
    applyLiveCaptionOpacity(state.opacity)
    let positionChanged = applyLiveCaptionPosition(state.position)
    let minimizedChanged = applyLiveCaptionMinimized(state.minimized)
    return positionChanged || minimizedChanged
  }

  func setFloatingBarOpacity(_ value: Double) {
    let nextValue = clampedFloatingBarOpacity(value)
    guard floatingBarOpacity != nextValue else { return }
    floatingBarOpacity = nextValue
    pendingFloatingBarOpacity = nextValue
    RustBridge.floatingBarSettingsChanged(
      FloatingOverlaySettingsChangePayload(floatingBarOpacity: nextValue))
  }

  func setLiveCaptionOpacity(_ value: Double) {
    let nextValue = clampedLiveCaptionOpacity(value)
    guard liveCaptionOpacity != nextValue else { return }
    liveCaptionOpacity = nextValue
    pendingLiveCaptionOpacity = nextValue
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

  private func applyFloatingBarOpacity(_ value: Double) {
    let nextValue = clampedFloatingBarOpacity(value)
    if let pendingFloatingBarOpacity {
      guard opacitiesMatch(pendingFloatingBarOpacity, nextValue) else { return }
      self.pendingFloatingBarOpacity = nil
    }

    floatingBarOpacity = nextValue
  }

  private func applyLiveCaptionOpacity(_ value: Double) {
    let nextValue = clampedLiveCaptionOpacity(value)
    if let pendingLiveCaptionOpacity {
      guard opacitiesMatch(pendingLiveCaptionOpacity, nextValue) else { return }
      self.pendingLiveCaptionOpacity = nil
    }

    liveCaptionOpacity = nextValue
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

  private func clampedFloatingBarOpacity(_ value: Double) -> Double {
    min(max(value, FloatingOverlayOpacity.minFloatingBar), FloatingOverlayOpacity.maxFloatingBar)
  }

  private func clampedLiveCaptionOpacity(_ value: Double) -> Double {
    min(max(value, FloatingOverlayOpacity.minLiveCaption), FloatingOverlayOpacity.maxLiveCaption)
  }

  private func opacitiesMatch(_ left: Double, _ right: Double) -> Bool {
    abs(left - right) < 0.0001
  }
}
