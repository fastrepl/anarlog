import Cocoa

final class LiveCaptionPanelDelegate: NSObject, NSWindowDelegate {
  private let placement = FloatingPanelPositionController()
  private let model: LiveCaptionViewModel

  init(model: LiveCaptionViewModel) {
    self.model = model
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
    let width = min(max(frameSize.width, LiveCaptionLayout.minWidth), LiveCaptionLayout.maxWidth)
    let lineCount = LiveCaptionLayout.lineCount(forHeight: frameSize.height)
    let height = LiveCaptionLayout.height(forLineCount: lineCount)

    DispatchQueue.main.async { [weak self] in
      self?.model.lineCount = lineCount
    }

    return NSSize(width: width, height: height)
  }
}
