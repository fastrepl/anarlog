import Cocoa
import SwiftUI

final class OverlayManager {
  static let shared = OverlayManager()

  private var panel: NSPanel?
  let model = OverlayViewModel()

  private init() {}

  func show() {
    DispatchQueue.main.async { [weak self] in
      guard let self else { return }
      if self.panel != nil {
        return
      }
      let panel = self.createPanel()
      let hosting = NSHostingView(rootView: OverlayView(model: self.model))
      hosting.frame = NSRect(
        x: 0, y: 0, width: OverlayLayout.containerWidth, height: OverlayLayout.containerHeight)
      hosting.autoresizingMask = [.width, .height]
      panel.contentView = hosting
      self.positionPanel(panel)
      panel.orderFrontRegardless()
      self.panel = panel
      self.fadeIn(panel)
    }
  }

  func hide() {
    DispatchQueue.main.async { [weak self] in
      guard let self, let panel = self.panel else { return }
      self.panel = nil
      self.fadeOut(panel) {
        panel.orderOut(nil)
      }
    }
  }

  func update(state: DictationStatePayload) {
    DispatchQueue.main.async { [weak self] in
      guard let self else { return }
      self.model.phase = state.phase
      let target = max(0, min(1, state.amplitude))
      let current = self.model.amplitude
      let lerped = current + Float(OverlayTiming.amplitudeLerp) * (target - current)
      self.model.amplitude = lerped
    }
  }
}
