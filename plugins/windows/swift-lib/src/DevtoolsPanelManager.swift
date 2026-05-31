import Cocoa
import SwiftUI

final class DevtoolsPanelManager {
  static let shared = DevtoolsPanelManager()

  private var panel: NSPanel?
  private var activeScreenId: CGDirectDisplayID?
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

      let panel = self.createPanel()
      let hostingView = NSHostingView(rootView: DevtoolsPanelView())
      hostingView.frame = NSRect(
        x: 0,
        y: 0,
        width: DevtoolsPanelLayout.containerWidth,
        height: DevtoolsPanelLayout.containerHeight)
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
      self.activeScreenId = nil
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
    return panel
  }

  private func position(_ panel: NSPanel, force: Bool = false) {
    let screen = activeScreen()
    let screenId = displayId(for: screen)
    if !force, screenId == activeScreenId {
      return
    }

    let frame = screen.visibleFrame
    let x = frame.maxX - DevtoolsPanelLayout.containerWidth - DevtoolsPanelLayout.screenMargin
    let y = frame.maxY - DevtoolsPanelLayout.containerHeight - DevtoolsPanelLayout.screenMargin
    panel.setFrameOrigin(NSPoint(x: x, y: y))
    activeScreenId = screenId
  }

  private func activeScreen() -> NSScreen {
    let mouse = NSEvent.mouseLocation
    let screens = NSScreen.screens

    if let exactScreen = screens.first(where: { $0.frame.contains(mouse) }) {
      return exactScreen
    }

    if let activeScreenId,
      let currentScreen = screens.first(where: { displayId(for: $0) == activeScreenId }),
      currentScreen.frame.insetBy(dx: -80, dy: -80).contains(mouse)
    {
      return currentScreen
    }

    return screens.min { left, right in
      distanceSquared(from: mouse, to: left.frame) < distanceSquared(from: mouse, to: right.frame)
    } ?? NSScreen.main ?? screens.first!
  }

  private func displayId(for screen: NSScreen) -> CGDirectDisplayID? {
    let key = NSDeviceDescriptionKey("NSScreenNumber")
    return (screen.deviceDescription[key] as? NSNumber).map { CGDirectDisplayID($0.uint32Value) }
  }

  private func distanceSquared(from point: NSPoint, to rect: NSRect) -> CGFloat {
    let clampedX = min(max(point.x, rect.minX), rect.maxX)
    let clampedY = min(max(point.y, rect.minY), rect.maxY)
    let dx = point.x - clampedX
    let dy = point.y - clampedY
    return dx * dx + dy * dy
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
