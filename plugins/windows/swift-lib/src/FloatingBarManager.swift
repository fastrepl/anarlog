import Cocoa
import Combine
import SwiftUI

final class FloatingBarManager {
  static let shared = FloatingBarManager()

  private var panel: NSPanel?
  private let model = FloatingBarViewModel()
  private let settingsModel = FloatingOverlaySettingsModel.shared
  private let placement = FloatingPanelPositionController()
  private var displayChangeObserver: Any?
  private var followActiveScreenTimer: Timer?
  private var isApplyingExternalState = false
  private var cancellables = Set<AnyCancellable>()

  private init() {
    model.$isExpanded
      .removeDuplicates()
      .sink { [weak self] _ in
        guard let self, let panel = self.panel else { return }
        guard !self.isApplyingExternalState else { return }
        let didResize = self.resize(panel)
        if !didResize {
          self.position(panel, force: true)
        }
      }
      .store(in: &cancellables)
  }

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
      let hostingView = NSHostingView(
        rootView: FloatingBarView(
          model: self.model,
          settings: self.settingsModel,
          panelOrigin: { [weak self] in self?.panel?.frame.origin },
          movePanel: { [weak self] origin in
            guard let self, let panel = self.panel else { return }
            self.placement.moveByUserDrag(panel, to: origin)
          }))
      hostingView.frame = NSRect(
        x: 0,
        y: 0,
        width: self.currentSize.width,
        height: self.currentSize.height)
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
      FloatingOverlaySettingsPanelManager.shared.hide()
      panel.orderOut(nil)
      self.panel = nil
      self.placement.resetActiveScreen()
    }
  }

  func update(state: FloatingBarStatePayload) {
    DispatchQueue.main.async { [weak self] in
      guard let self else { return }
      self.isApplyingExternalState = true
      self.model.status = state.status
      self.model.amplitude = min(max(state.amplitude, 0), 1)
      self.model.colorScheme = state.colorScheme
      self.model.title = state.title
      self.model.liveCaptionToggleVisible = state.liveCaptionToggleVisible
      self.model.transcriptBubbles = state.transcriptBubbles
      self.settingsModel.apply(floatingBarState: state)
      self.model.isExpanded =
        state.liveCaptionToggleVisible && !self.settingsModel.liveCaptionMinimized
      self.isApplyingExternalState = false
      if let panel = self.panel {
        let didResize = self.resize(panel)
        if !didResize {
          self.position(panel, force: true)
        }
      }
    }
  }

  private func createPanel() -> NSPanel {
    let panel = NSPanel(
      contentRect: NSRect(
        x: 0,
        y: 0,
        width: currentSize.width,
        height: currentSize.height),
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
    panel.minSize = currentSize
    panel.delegate = placement
    return panel
  }

  private func position(_ panel: NSPanel, force: Bool = false) {
    let size = currentSize
    placement.position(
      panel,
      force: force,
      size: size
    ) { screen, size in
      let frame = screen.visibleFrame
      let x = frame.maxX - size.width - FloatingBarLayout.screenMargin
      let y = frame.maxY - size.height - FloatingBarLayout.screenMargin
      return NSPoint(x: x, y: y)
    }
  }

  private func resize(_ panel: NSPanel) -> Bool {
    let size = currentSize
    let previousSize = panel.frame.size
    panel.minSize = size
    guard previousSize != size else { return false }

    let previousLayout =
      layout(matching: previousSize)
      ?? FloatingBarWindowLayout(
        isExpanded: !model.isExpanded,
        showsExpand: model.liveCaptionToggleVisible)
    let nextLayout = currentLayout
    let previousAnchorOffset = controlAnchorOffset(for: previousLayout)
    let nextAnchorOffset = controlAnchorOffset(for: nextLayout)
    let anchor = NSPoint(
      x: panel.frame.minX + previousAnchorOffset.x,
      y: panel.frame.minY + previousAnchorOffset.y
    )
    let frame = NSRect(
      x: anchor.x - nextAnchorOffset.x,
      y: anchor.y - nextAnchorOffset.y,
      width: size.width,
      height: size.height)
    placement.setFrame(panel, to: frame, display: true, animate: false)
    panel.contentView?.frame = NSRect(origin: .zero, size: size)
    return true
  }

  private var currentSize: NSSize {
    size(for: currentLayout)
  }

  private var currentLayout: FloatingBarWindowLayout {
    FloatingBarWindowLayout(
      isExpanded: model.isExpanded,
      showsExpand: model.liveCaptionToggleVisible
    )
  }

  private func size(for layout: FloatingBarWindowLayout) -> NSSize {
    FloatingBarLayout.containerSize(
      isExpanded: layout.isExpanded,
      showsExpand: layout.showsExpand
    )
  }

  private func layout(matching size: NSSize) -> FloatingBarWindowLayout? {
    let candidates = [
      FloatingBarWindowLayout(isExpanded: true, showsExpand: true),
      FloatingBarWindowLayout(isExpanded: true, showsExpand: false),
      FloatingBarWindowLayout(isExpanded: false, showsExpand: true),
      FloatingBarWindowLayout(isExpanded: false, showsExpand: false),
    ]

    return candidates.first { candidate in
      let candidateSize = self.size(for: candidate)
      return abs(candidateSize.width - size.width) < 0.5
        && abs(candidateSize.height - size.height) < 0.5
    }
  }

  private func controlAnchorOffset(for layout: FloatingBarWindowLayout) -> NSPoint {
    if layout.isExpanded {
      return NSPoint(
        x: FloatingBarLayout.inset + FloatingBarLayout.expandedWidth
          - FloatingBarLayout.compactHorizontalPadding,
        y: FloatingBarLayout.inset + FloatingBarLayout.expandedHeight
      )
    }

    return NSPoint(
      x: FloatingBarLayout.inset + FloatingBarLayout.compactHorizontalPadding
        + FloatingBarLayout.compactControlsWidth(showsExpand: layout.showsExpand),
      y: FloatingBarLayout.inset + FloatingBarLayout.compactHeight
    )
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

private struct FloatingBarWindowLayout {
  let isExpanded: Bool
  let showsExpand: Bool
}
