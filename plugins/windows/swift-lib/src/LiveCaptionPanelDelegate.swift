import Cocoa

final class LiveCaptionPanelDelegate: NSObject, NSWindowDelegate {
  private let placement = FloatingPanelPositionController()
  private let model: LiveCaptionViewModel
  private let settings: FloatingOverlaySettingsModel

  init(model: LiveCaptionViewModel, settings: FloatingOverlaySettingsModel) {
    self.model = model
    self.settings = settings
  }

  func position(
    _ panel: NSPanel,
    force: Bool = false,
    defaultOrigin: (NSScreen, NSSize) -> NSPoint
  ) {
    placement.position(panel, force: force, size: panel.frame.size, defaultOrigin: defaultOrigin)
  }

  func resetActiveScreen() {
    placement.resetActiveScreen()
  }

  func windowDidMove(_ notification: Notification) {
    placement.windowDidMove(notification)
  }

  func windowWillResize(_ sender: NSWindow, to frameSize: NSSize) -> NSSize {
    if settings.liveCaptionMinimized {
      return LiveCaptionLayout.minimizedSize
    }

    let width = min(max(frameSize.width, LiveCaptionLayout.minWidth), LiveCaptionLayout.maxWidth)
    let lineCount = LiveCaptionLayout.lineCount(forHeight: frameSize.height)
    let height = LiveCaptionLayout.height(forLineCount: lineCount)

    DispatchQueue.main.async { [weak self] in
      guard let self else { return }
      model.lineCount = lineCount
      settings.setLiveCaptionSize(width: Double(width), lineCount: lineCount)
    }

    return NSSize(width: width, height: height)
  }

  func clearPinnedPosition() {
    placement.clearPinnedOrigin()
  }

  func setFrame(_ panel: NSPanel, to frame: NSRect, display: Bool, animate: Bool) {
    placement.setFrame(panel, to: frame, display: display, animate: animate)
  }

}
