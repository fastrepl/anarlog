import Cocoa
import SwiftUI

final class FloatingBarManager {
  static let shared = FloatingBarManager()

  private var panel: NSPanel?
  private let model = FloatingBarViewModel()
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

      FloatingBarFonts.register()

      let panel = self.createPanel()
      let hostingView = NSHostingView(rootView: FloatingBarView(model: self.model))
      hostingView.frame = NSRect(
        x: 0,
        y: 0,
        width: FloatingBarLayout.containerWidth,
        height: FloatingBarLayout.containerHeight(
          disclosureVisible: self.model.disclosureVisible))
      hostingView.autoresizingMask = [.width, .height]

      panel.contentView = hostingView
      self.resize(panel, disclosureVisible: self.model.disclosureVisible)
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

  func update(state: FloatingBarStatePayload) {
    DispatchQueue.main.async { [weak self] in
      guard let self else { return }
      self.model.degraded = state.degraded
      self.model.amplitude = min(max(state.amplitude, 0), 1)
      self.model.disclosureVisible = state.disclosureVisible
      if let panel = self.panel {
        self.resize(panel, disclosureVisible: state.disclosureVisible)
      }
    }
  }

  private func createPanel() -> NSPanel {
    let panel = NSPanel(
      contentRect: NSRect(
        x: 0,
        y: 0,
        width: FloatingBarLayout.containerWidth,
        height: FloatingBarLayout.containerHeight(
          disclosureVisible: model.disclosureVisible)),
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
    return panel
  }

  private func resize(_ panel: NSPanel, disclosureVisible: Bool) {
    let size = NSSize(
      width: FloatingBarLayout.containerWidth,
      height: FloatingBarLayout.containerHeight(disclosureVisible: disclosureVisible)
    )

    if panel.frame.size == size {
      return
    }

    panel.setContentSize(size)
    panel.contentView?.frame = NSRect(origin: .zero, size: size)
    position(panel, force: true)
  }

  private func position(_ panel: NSPanel, force: Bool = false) {
    let screen = activeScreen()
    let screenId = displayId(for: screen)
    if !force, screenId == activeScreenId {
      return
    }

    let frame = screen.visibleFrame
    let x = frame.maxX - FloatingBarLayout.containerWidth - FloatingBarLayout.screenMargin
    let y = frame.midY - panel.frame.height / 2
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
