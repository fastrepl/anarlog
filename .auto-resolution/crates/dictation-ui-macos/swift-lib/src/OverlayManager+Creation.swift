import Cocoa

extension OverlayManager {
  func createPanel() -> NSPanel {
    let panel = NSPanel(
      contentRect: NSRect(
        x: 0, y: 0, width: OverlayLayout.containerWidth, height: OverlayLayout.containerHeight),
      styleMask: [.borderless, .nonactivatingPanel],
      backing: .buffered,
      defer: false
    )

    panel.level = .statusBar
    panel.isFloatingPanel = true
    panel.hidesOnDeactivate = false
    panel.isOpaque = false
    panel.backgroundColor = .clear
    panel.hasShadow = false
    panel.collectionBehavior = [
      .canJoinAllSpaces, .fullScreenAuxiliary, .ignoresCycle, .stationary,
    ]
    panel.isMovableByWindowBackground = false
    panel.ignoresMouseEvents = true
    panel.alphaValue = 0
    return panel
  }

  func activeScreen() -> NSScreen {
    let mouse = NSEvent.mouseLocation
    for screen in NSScreen.screens where screen.frame.contains(mouse) {
      return screen
    }
    return NSScreen.main ?? NSScreen.screens.first!
  }

  func positionPanel(_ panel: NSPanel) {
    let screen = activeScreen()
    let frame = screen.visibleFrame
    let x = frame.midX - OverlayLayout.containerWidth / 2
    let y = frame.minY + OverlayLayout.bottomInset - OverlayLayout.shadowPadding
    panel.setFrameOrigin(NSPoint(x: x, y: y))
  }
}
