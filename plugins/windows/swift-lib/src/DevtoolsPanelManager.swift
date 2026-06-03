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
        return
      }

      let panel = self.createPanel()
      let hostingView = NSHostingView(
        rootView: DevtoolsPanelView { [weak self, weak panel] isCollapsed in
          guard let panel else { return }
          self?.resize(panel, isCollapsed: isCollapsed)
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
    panel.hasShadow = true
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

  private func resize(_ panel: NSPanel, isCollapsed: Bool) {
    let height =
      isCollapsed
      ? DevtoolsPanelLayout.collapsedHeight
      : DevtoolsPanelLayout.containerHeight
    let size = NSSize(width: DevtoolsPanelLayout.containerWidth, height: height)
    targetPanelSize = size
    guard abs(panel.frame.height - height) > 0.5 else { return }

    let frame = NSRect(
      x: panel.frame.minX,
      y: panel.frame.maxY - height,
      width: size.width,
      height: size.height)
    placement.setFrame(panel, to: frame, display: true, animate: true)
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
