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
        self.resize(panel, to: layout)
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
      position(panel, force: true, followsPointer: model.dictation != nil)
      startObservingDisplayChanges()
      panel.orderFrontRegardless()
      return
    }

    model.placement = nil
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
    position(panel, force: true, followsPointer: model.dictation != nil)
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
    model.dictation = state.dictation
    (panel as? FloatingBarPanel)?.dictationMode = state.dictation != nil
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
    if state.dictation == nil {
      settingsModel.apply(floatingBarState: state)
    }
    let minimized =
      state.dictation == nil ? settingsModel.liveCaptionMinimized : state.liveCaptionMinimized
    let isExpanded = state.liveCaptionToggleVisible && !minimized
    if model.isExpanded != isExpanded {
      model.isExpanded = isExpanded
    }
    isApplyingExternalState = false
    if let panel {
      resize(panel)
    }
  }

  private func applyAmplitude(_ amplitude: Double) {
    let amplitude = min(max(amplitude, 0), 1)
    guard model.amplitude != amplitude else { return }
    model.amplitude = amplitude
  }

  private func createPanel() -> NSPanel {
    let panel = FloatingBarPanel(
      contentRect: NSRect(
        x: 0,
        y: 0,
        width: currentSize.width,
        height: currentSize.height),
      styleMask: [.borderless, .nonactivatingPanel, .resizable],
      backing: .buffered,
      defer: false
    )

    panel.dictationMode = model.dictation != nil
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
    followsPointer: Bool = false,
    layout targetLayout: FloatingBarWindowLayout? = nil
  ) {
    let layout = targetLayout ?? currentLayout
    let size = model.placement?.frame.size ?? size(for: layout)
    placement.position(
      panel,
      force: force,
      size: size,
      anchorOffset: controlAnchorOffset(for: layout),
      followsPointer: followsPointer
    ) { screen, size in
      let frame = screen.visibleFrame
      let x = frame.midX - size.width / 2
      let y = frame.minY + FloatingBarLayout.screenMargin
      return NSPoint(x: x, y: y)
    }
    resize(panel, to: layout)
  }

  private func resize(_ panel: NSPanel, to targetLayout: FloatingBarWindowLayout? = nil) {
    let nextLayout = targetLayout ?? currentLayout
    let requestedSize = size(for: nextLayout)
    let offset = controlAnchorOffset(for: nextLayout)
    let anchor = NSPoint(x: panel.frame.minX + offset.x, y: panel.frame.minY + offset.y)
    let workArea = (panel.screen ?? NSScreen.main)?.visibleFrame ?? panel.frame
    let grows =
      nextLayout.isExpanded
      && panel.frame.height
        <= FloatingBarLayout.containerSize(isExpanded: false, showsExpand: true).height
    let expandsUpward =
      grows
      ? workArea.maxY - anchor.y > anchor.y - workArea.minY
      : model.placement?.expandsUpward ?? true
    let next = FloatingControlPlacement.layout(
      anchor: anchor, size: requestedSize,
      workArea: workArea, expandsUpward: expandsUpward)
    model.placement = next
    panel.minSize = next.frame.size
    placement.setFrame(
      panel, to: next.frame, display: true, animate: false, anchorOffset: next.controlOffset)
    panel.contentView?.frame = NSRect(origin: .zero, size: next.frame.size)
  }

  private var currentSize: NSSize {
    model.placement?.frame.size ?? size(for: currentLayout)
  }

  private var currentLayout: FloatingBarWindowLayout {
    layout(isExpanded: model.isExpanded)
  }

  private func layout(isExpanded: Bool) -> FloatingBarWindowLayout {
    FloatingBarWindowLayout(
      isExpanded: isExpanded,
      showsExpand: model.liveCaptionToggleVisible,
      isDictation: model.dictation != nil
    )
  }

  private func size(for layout: FloatingBarWindowLayout) -> NSSize {
    FloatingBarLayout.containerSize(
      isExpanded: layout.isExpanded,
      showsExpand: layout.showsExpand,
      isDictation: layout.isDictation
    )
  }

  private func controlAnchorOffset(for layout: FloatingBarWindowLayout) -> NSPoint {
    model.placement?.controlOffset
      ?? NSPoint(
        x: (panel?.frame.width ?? size(for: layout).width) / 2,
        y: FloatingBarLayout.inset + FloatingBarLayout.compactHeight / 2)
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
  let isDictation: Bool
}

final class FloatingBarPanel: NSPanel {
  var dictationMode = false
  override var canBecomeKey: Bool { !dictationMode && super.canBecomeKey }
  override var canBecomeMain: Bool { false }
}
