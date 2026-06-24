import Cocoa
import SwiftUI

final class LiveCaptionManager {
  static let shared = LiveCaptionManager()

  private var panel: NSPanel?
  private let model = LiveCaptionViewModel()
  private let settingsModel = FloatingOverlaySettingsModel.shared
  private lazy var panelDelegate = LiveCaptionPanelDelegate(
    model: model,
    settings: settingsModel)
  private var displayChangeObserver: Any?
  private var followActiveScreenTimer: Timer?

  private init() {}

  func show() {
    runOnMain { [weak self] in
      guard let self else { return }
      guard !self.settingsModel.liveCaptionMinimized else {
        self.hidePanel(clearText: false)
        return
      }

      if let panel = self.panel {
        self.resize(panel)
        self.position(panel, force: true)
        self.startFollowingActiveScreen()
        panel.orderFrontRegardless()
        return
      }

      let panel = self.createPanel()
      let hostingView = NSHostingView(
        rootView: LiveCaptionView(
          model: self.model,
          settings: self.settingsModel,
          onSetMinimized: { [weak self] minimized in
            self?.setMinimized(minimized)
          }))
      hostingView.frame = NSRect(
        x: 0,
        y: 0,
        width: self.initialSize.width,
        height: self.initialSize.height)
      hostingView.autoresizingMask = [.width, .height]

      panel.contentView = hostingView
      self.resize(panel)
      self.position(panel, force: true)
      panel.orderFrontRegardless()
      self.panel = panel
      self.startFollowingActiveScreen()
    }
  }

  func hide(clearText: Bool = true) {
    runOnMain { [weak self] in
      guard let self else { return }
      self.hidePanel(clearText: clearText)
    }
  }

  private func hidePanel(clearText: Bool = true) {
    guard let panel else {
      if clearText {
        model.text = ""
      }
      panelDelegate.resetActiveScreen()
      return
    }

    stopFollowingActiveScreen()
    FloatingOverlaySettingsPanelManager.shared.hide()
    panel.orderOut(nil)
    self.panel = nil
    panelDelegate.resetActiveScreen()
    if clearText {
      model.text = ""
    }
    model.lineCount = LiveCaptionLayout.defaultLineCount
  }

  func update(state: LiveCaptionStatePayload) {
    runOnMain { [weak self] in
      guard let self else { return }
      let placementChanged: Bool
      if self.panel == nil {
        placementChanged = self.settingsModel.apply(liveCaptionState: state)
      } else {
        placementChanged = self.settingsModel.applyLiveCaptionLayout(state)
      }
      if state.minimized {
        self.hidePanel(clearText: false)
        return
      }

      self.model.text = state.text
      if let panel = self.panel {
        self.resize(panel)
        guard NSEvent.pressedMouseButtons == 0 else { return }
        if placementChanged {
          self.panelDelegate.clearPinnedPosition()
        }
        self.position(panel, force: true)
      }
    }
  }

  private func createPanel() -> NSPanel {
    let initialSize = initialSize
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
      self.settingsModel.liveCaptionPosition.origin(in: screen.visibleFrame, size: size)
    }
  }

  private var initialSize: NSSize {
    return NSSize(
      width: CGFloat(settingsModel.liveCaptionWidth),
      height: LiveCaptionLayout.height(forLineCount: settingsModel.liveCaptionLineCount))
  }

  private func resize(_ panel: NSPanel) {
    let targetSize = targetSize(for: panel)
    updateSizeConstraints(panel, targetSize: targetSize)

    guard panel.frame.size != targetSize else { return }

    panelDelegate.setFrame(
      panel,
      to: NSRect(
        x: panel.frame.minX,
        y: panel.frame.maxY - targetSize.height,
        width: targetSize.width,
        height: targetSize.height),
      display: true,
      animate: false)
  }

  private func targetSize(for panel: NSPanel) -> NSSize {
    let isResizing = NSEvent.pressedMouseButtons != 0
    let requestedWidth = isResizing ? panel.frame.width : CGFloat(settingsModel.liveCaptionWidth)
    let requestedLineCount = isResizing ? model.lineCount : settingsModel.liveCaptionLineCount
    let width = min(max(requestedWidth, LiveCaptionLayout.minWidth), LiveCaptionLayout.maxWidth)
    let lineCount = min(
      max(requestedLineCount, LiveCaptionLayout.minLineCount),
      LiveCaptionLayout.maxLineCount
    )
    model.lineCount = lineCount

    return NSSize(
      width: width,
      height: LiveCaptionLayout.height(forLineCount: lineCount))
  }

  private func updateSizeConstraints(_ panel: NSPanel, targetSize: NSSize) {
    panel.minSize = NSSize(
      width: LiveCaptionLayout.minWidth,
      height: LiveCaptionLayout.height(forLineCount: LiveCaptionLayout.minLineCount))
    panel.maxSize = NSSize(
      width: LiveCaptionLayout.maxWidth,
      height: LiveCaptionLayout.height(forLineCount: LiveCaptionLayout.maxLineCount))
  }

  private func setMinimized(_ minimized: Bool) {
    settingsModel.setLiveCaptionMinimized(minimized)
    if minimized {
      hidePanel(clearText: false)
      return
    }

    if panel == nil {
      show()
      return
    }

    guard let panel else { return }
    resize(panel)
    panelDelegate.clearPinnedPosition()
    position(panel, force: true)
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
