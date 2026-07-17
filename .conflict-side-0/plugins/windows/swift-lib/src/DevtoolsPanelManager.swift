import Cocoa
import SwiftUI

final class DevtoolsPanelManager {
  static let shared = DevtoolsPanelManager()

  private var panel: NSPanel?
  private let placement = FloatingPanelPositionController()
  private var displayChangeObserver: Any?
  private var followActiveScreenTimer: Timer?
  private var targetPanelSize = NSSize(
    width: DevtoolsPanelLayout.containerWidth,
    height: DevtoolsPanelLayout.containerHeight)

  private init() {}

  func show() {
    DispatchQueue.main.async { [weak self] in
      guard let self else { return }

      if let panel = self.panel {
        self.position(panel, force: true)
        self.startFollowingActiveScreen()
        panel.orderFrontRegardless()
        RustBridge.devtoolsPanelAction("panel:opened")
        return
      }

      let panel = self.createPanel()
      let hostingView = NSHostingView(
        rootView: DevtoolsPanelView { [weak self] in
          self?.hide()
        })
      hostingView.frame = NSRect(
        x: 0,
        y: 0,
        width: DevtoolsPanelLayout.containerWidth,
        height: DevtoolsPanelLayout.containerHeight)
      hostingView.autoresizingMask = [.width, .height]

      panel.contentView = hostingView
      self.targetPanelSize = NSSize(
        width: DevtoolsPanelLayout.containerWidth,
        height: DevtoolsPanelLayout.containerHeight)
      self.position(panel, force: true)
      panel.orderFrontRegardless()
      self.panel = panel
      self.startFollowingActiveScreen()
      RustBridge.devtoolsPanelAction("panel:opened")
    }
  }

  func hide() {
    DispatchQueue.main.async { [weak self] in
      guard let self, let panel = self.panel else { return }
      self.stopFollowingActiveScreen()
      self.placement.preparePinnedFrameForReplacement(
        panel,
        size: NSSize(
          width: DevtoolsPanelLayout.containerWidth,
          height: DevtoolsPanelLayout.containerHeight))
      panel.orderOut(nil)
      self.panel = nil
      self.targetPanelSize = NSSize(
        width: DevtoolsPanelLayout.containerWidth,
        height: DevtoolsPanelLayout.containerHeight)
      self.placement.resetActiveScreen()
      RustBridge.devtoolsPanelAction("panel:closed")
    }
  }

  private func createPanel() -> NSPanel {
    let panel = NSPanel(
      contentRect: NSRect(
        x: 0,
        y: 0,
        width: DevtoolsPanelLayout.containerWidth,
        height: DevtoolsPanelLayout.containerHeight),
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
    placement.position(panel, force: force, size: targetPanelSize) { screen, size in
      let frame = screen.visibleFrame
      let x = frame.maxX - size.width - DevtoolsPanelLayout.screenMargin
      let y = frame.maxY - size.height - DevtoolsPanelLayout.screenMargin
      return NSPoint(x: x, y: y)
    }
  }

  private func startFollowingActiveScreen() {
    guard followActiveScreenTimer == nil else { return }

    let timer = Timer(timeInterval: 0.35, repeats: true) { [weak self] _ in
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
