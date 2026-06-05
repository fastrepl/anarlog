import Cocoa
import SwiftUI

final class FloatingBarManager {
  static let shared = FloatingBarManager()

  private var panel: NSPanel?
  private let model = FloatingBarViewModel()
  private let placement = FloatingPanelPositionController()
  private var displayChangeObserver: Any?
  private var followActiveScreenTimer: Timer?

  private init() {}

  func show() {
    DispatchQueue.main.async { [weak self] in
      guard let self else { return }

      if let panel = self.panel {
        self.position(panel, force: true)
        self.startFollowingActiveScreen()
        panel.orderFrontRegardless()
        return
      }

      FloatingBarFonts.register()

      let panel = self.createPanel()
      let hostingView = NSHostingView(rootView: FloatingBarView(model: self.model))
      hostingView.frame = NSRect(
        x: 0,
        y: 0,
        width: FloatingBarLayout.containerWidth,
        height: FloatingBarLayout.containerHeight)
      hostingView.autoresizingMask = [.width, .height]

      panel.contentView = hostingView
      self.position(panel, force: true)
      panel.orderFrontRegardless()
      self.panel = panel
      self.startFollowingActiveScreen()
    }
  }

  func hide() {
    DispatchQueue.main.async { [weak self] in
      guard let self, let panel = self.panel else { return }
      self.stopFollowingActiveScreen()
      panel.orderOut(nil)
      self.panel = nil
      self.placement.resetActiveScreen()
    }
  }

  func update(state: FloatingBarStatePayload) {
    DispatchQueue.main.async { [weak self] in
      guard let self else { return }
      self.model.status = state.status
      self.model.amplitude = min(max(state.amplitude, 0), 1)
    }
  }

  private func createPanel() -> NSPanel {
    let panel = NSPanel(
      contentRect: NSRect(
        x: 0,
        y: 0,
        width: FloatingBarLayout.containerWidth,
        height: FloatingBarLayout.containerHeight),
      styleMask: [.borderless, .nonactivatingPanel],
      backing: .buffered,
      defer: false
    )

    panel.level = .floating
    panel.isFloatingPanel = true
    panel.hidesOnDeactivate = false
    panel.isOpaque = false
    panel.backgroundColor = .clear
    panel.hasShadow = false
    panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]
    panel.isMovableByWindowBackground = true
    panel.delegate = placement
    return panel
  }

  private func position(_ panel: NSPanel, force: Bool = false) {
    placement.position(
      panel,
      force: force,
      size: NSSize(
        width: FloatingBarLayout.containerWidth,
        height: FloatingBarLayout.containerHeight)
    ) { screen, size in
      let frame = screen.visibleFrame
      let x = frame.maxX - size.width - FloatingBarLayout.screenMargin
      let y = frame.midY - size.height / 2
      return NSPoint(x: x, y: y)
    }
  }

  private func startFollowingActiveScreen() {
    guard followActiveScreenTimer == nil else { return }

    let timer = Timer(timeInterval: 0.25, repeats: true) { [weak self] _ in
      guard let self, let panel = self.panel else { return }
      self.position(panel)
    }
    RunLoop.main.add(timer, forMode: .common)
    followActiveScreenTimer = timer

    displayChangeObserver = NotificationCenter.default.addObserver(
      forName: NSApplication.didChangeScreenParametersNotification,
      object: nil,
      queue: .main
    ) { [weak self] _ in
      guard let self, let panel = self.panel else { return }
      self.position(panel, force: true)
    }
  }

  private func stopFollowingActiveScreen() {
    followActiveScreenTimer?.invalidate()
    followActiveScreenTimer = nil

    if let displayChangeObserver {
      NotificationCenter.default.removeObserver(displayChangeObserver)
      self.displayChangeObserver = nil
    }
  }
}
