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
  private var isApplyingExternalState = false
  private var cancellables = Set<AnyCancellable>()
  private var commandCoalescer: FloatingBarCommandCoalescer!

  private init() {
    commandCoalescer = FloatingBarCommandCoalescer { [weak self] action in
      self?.apply(action)
    }
    model.$isExpanded
      .removeDuplicates()
      .sink { [weak self] isExpanded in
        guard let self, let panel = self.panel else { return }
        guard !self.isApplyingExternalState else { return }
        let layout = self.layout(isExpanded: isExpanded)
        let didResize = self.resize(panel, to: layout)
        if !didResize {
          self.position(panel, force: true, layout: layout)
        }
      }
      .store(in: &cancellables)
  }

  func show() {
    commandCoalescer.enqueueShow()
  }

  func hide() {
    commandCoalescer.enqueueHide()
  }

  func update(state: FloatingBarStatePayload) {
    commandCoalescer.enqueueUpdate(state)
  }

  func update(amplitude: Double) {
    commandCoalescer.enqueueAmplitude(amplitude)
  }

  private func apply(_ action: FloatingBarCommandCoalescer.Action) {
    dispatchPrecondition(condition: .onQueue(.main))

    switch action {
    case .show:
      applyShow()
    case .hide:
      applyHide()
    case .update(let state):
      applyUpdate(state)
    case .amplitude(let amplitude):
      applyAmplitude(amplitude)
    }
  }

  private func applyShow() {
    if let panel {
      position(panel, force: true)
      startObservingDisplayChanges()
      panel.orderFrontRegardless()
      return
    }

    FloatingBarFonts.register()

    let panel = createPanel()
    let hostingView = NSHostingView(
      rootView: FloatingBarView(
        model: model,
        settings: settingsModel,
        panelOrigin: { [weak self] in self?.panel?.frame.origin },
        movePanel: { [weak self] origin in
          guard let self, let panel = self.panel else { return }
          self.placement.moveByUserDrag(
            panel,
            to: origin,
            anchorOffset: self.controlAnchorOffset(for: self.currentLayout))
        }))
    hostingView.frame = NSRect(
      x: 0,
      y: 0,
      width: currentSize.width,
      height: currentSize.height)
    hostingView.autoresizingMask = [.width, .height]

    panel.contentView = hostingView
    position(panel, force: true)
    panel.orderFrontRegardless()
    self.panel = panel
    startObservingDisplayChanges()
  }

  private func applyHide() {
    guard let panel else { return }
    stopObservingDisplayChanges()
    FloatingOverlaySettingsPanelManager.shared.hide()
    panel.orderOut(nil)
    self.panel = nil
    placement.resetActiveScreen()
  }

  private func applyUpdate(_ state: FloatingBarStatePayload) {
    isApplyingExternalState = true
    if model.status != state.status {
      model.status = state.status
    }
    applyAmplitude(state.amplitude)
    if model.colorScheme != state.colorScheme {
      model.colorScheme = state.colorScheme
    }
    if model.title != state.title {
      model.title = state.title
    }
    if model.liveCaptionToggleVisible != state.liveCaptionToggleVisible {
      model.liveCaptionToggleVisible = state.liveCaptionToggleVisible
    }
    if let transcriptBubbles = state.transcriptBubbles,
      model.transcriptBubbles != transcriptBubbles
    {
      model.transcriptBubbles = transcriptBubbles
    }
    settingsModel.apply(floatingBarState: state)
    let isExpanded =
      state.liveCaptionToggleVisible && !settingsModel.liveCaptionMinimized
    if model.isExpanded != isExpanded {
      model.isExpanded = isExpanded
    }
    isApplyingExternalState = false
    if let panel {
      let didResize = resize(panel)
      if !didResize {
        position(panel, force: true)
      }
    }
  }

  private func applyAmplitude(_ amplitude: Double) {
    let amplitude = min(max(amplitude, 0), 1)
    guard model.amplitude != amplitude else { return }
    model.amplitude = amplitude
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

  private func position(
    _ panel: NSPanel,
    force: Bool = false,
    layout targetLayout: FloatingBarWindowLayout? = nil
  ) {
    let layout = targetLayout ?? currentLayout
    let size = size(for: layout)
    placement.position(
      panel,
      force: force,
      size: size,
      anchorOffset: controlAnchorOffset(for: layout),
      followsPointer: false
    ) { screen, size in
      let frame = screen.visibleFrame
      let x = frame.maxX - size.width - FloatingBarLayout.screenMargin
      let y = frame.maxY - size.height - FloatingBarLayout.screenMargin
      return NSPoint(x: x, y: y)
    }
  }

  private func resize(
    _ panel: NSPanel,
    to targetLayout: FloatingBarWindowLayout? = nil
  ) -> Bool {
    let nextLayout = targetLayout ?? currentLayout
    let size = size(for: nextLayout)
    let previousSize = panel.frame.size
    panel.minSize = size
    guard previousSize != size else { return false }

    let previousLayout =
      layout(matching: previousSize)
      ?? FloatingBarWindowLayout(
        isExpanded: !nextLayout.isExpanded,
        showsExpand: model.liveCaptionToggleVisible)
    let previousAnchorOffset = controlAnchorOffset(for: previousLayout)
    let nextAnchorOffset = controlAnchorOffset(for: nextLayout)
    let anchor = placement.anchorPoint(for: panel, offset: previousAnchorOffset)
    let frame = NSRect(
      x: anchor.x - nextAnchorOffset.x,
      y: anchor.y - nextAnchorOffset.y,
      width: size.width,
      height: size.height)
    placement.setFrame(
      panel,
      to: frame,
      display: true,
      animate: false,
      anchorOffset: nextAnchorOffset)
    panel.contentView?.frame = NSRect(origin: .zero, size: size)
    return true
  }

  private var currentSize: NSSize {
    size(for: currentLayout)
  }

  private var currentLayout: FloatingBarWindowLayout {
    layout(isExpanded: model.isExpanded)
  }

  private func layout(isExpanded: Bool) -> FloatingBarWindowLayout {
    FloatingBarWindowLayout(
      isExpanded: isExpanded,
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

  private func startObservingDisplayChanges() {
    guard displayChangeObserver == nil else { return }

    displayChangeObserver = NotificationCenter.default.addObserver(
      forName: NSApplication.didChangeScreenParametersNotification,
      object: nil,
      queue: .main
    ) { [weak self] _ in
      guard let self, let panel = self.panel else { return }
      self.position(panel, force: true)
    }
  }

  private func stopObservingDisplayChanges() {
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
