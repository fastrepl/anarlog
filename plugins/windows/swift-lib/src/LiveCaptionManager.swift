import Cocoa
import SwiftUI

final class LiveCaptionManager {
  static let shared = LiveCaptionManager()

  private var panel: NSPanel?
  private let model = LiveCaptionViewModel()
  private lazy var panelDelegate = LiveCaptionPanelDelegate(model: model)
  private var displayChangeObserver: Any?
  private var followActiveScreenTimer: Timer?

  private init() {}

  func show() {
    runOnMain { [weak self] in
      guard let self else { return }

      if let panel = self.panel {
        self.position(panel, force: true)
        self.startFollowingActiveScreen()
        panel.orderFrontRegardless()
        return
      }

      let panel = self.createPanel()
      let hostingView = NSHostingView(rootView: LiveCaptionView(model: self.model))
      hostingView.frame = NSRect(
        x: 0,
        y: 0,
        width: LiveCaptionLayout.defaultWidth,
        height: LiveCaptionLayout.height(forLineCount: LiveCaptionLayout.defaultLineCount))
      hostingView.autoresizingMask = [.width, .height]

      panel.contentView = hostingView
      self.position(panel, force: true)
      panel.orderFrontRegardless()
      self.panel = panel
      self.startFollowingActiveScreen()
    }
  }

  func hide() {
    runOnMain { [weak self] in
      guard let self else { return }
      self.hidePanel()
    }
  }

  private func hidePanel() {
    guard let panel else {
      model.text = ""
      panelDelegate.resetActiveScreen()
      return
    }

    stopFollowingActiveScreen()
    panel.orderOut(nil)
    self.panel = nil
    panelDelegate.resetActiveScreen()
    model.text = ""
  }

  func update(state: LiveCaptionStatePayload) {
    runOnMain { [weak self] in
      guard let self else { return }
      self.model.text = state.text
      self.model.opacity = min(max(state.opacity, 0.35), 0.95)
    }
  }

  private func createPanel() -> NSPanel {
    let initialSize = NSSize(
      width: LiveCaptionLayout.defaultWidth,
      height: LiveCaptionLayout.height(forLineCount: LiveCaptionLayout.defaultLineCount))
    let panel = NSPanel(
      contentRect: NSRect(origin: .zero, size: initialSize),
      styleMask: [.borderless, .nonactivatingPanel, .resizable],
      backing: .buffered,
      defer: false
    )

    panel.level = .floating
    panel.isFloatingPanel = true
    panel.hidesOnDeactivate = false
    panel.isOpaque = false
    panel.backgroundColor = .clear
    panel.hasShadow = false
    panel.sharingType = .none
    panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]
    panel.isMovableByWindowBackground = true
    panel.minSize = NSSize(
      width: LiveCaptionLayout.minWidth,
      height: LiveCaptionLayout.height(forLineCount: LiveCaptionLayout.minLineCount))
    panel.maxSize = NSSize(
      width: LiveCaptionLayout.maxWidth,
      height: LiveCaptionLayout.height(forLineCount: LiveCaptionLayout.maxLineCount))
    panel.delegate = panelDelegate
    return panel
  }

  private func runOnMain(_ block: @escaping () -> Void) {
    if Thread.isMainThread {
      block()
      return
    }

    DispatchQueue.main.sync(execute: block)
  }

  private func position(_ panel: NSPanel, force: Bool = false) {
    panelDelegate.position(panel, force: force) { screen, size in
      let frame = screen.visibleFrame
      let x = frame.midX - size.width / 2
      let y = frame.maxY - size.height - LiveCaptionLayout.topOffset
      return NSPoint(
        x: min(max(x, frame.minX + LiveCaptionLayout.screenMargin), frame.maxX - size.width),
        y: max(y, frame.minY + LiveCaptionLayout.screenMargin)
      )
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
