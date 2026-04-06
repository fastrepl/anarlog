import AppKit
import SwiftUI

final class OverlayPanel: NSPanel {
  init(contentView: NSView) {
    super.init(
      contentRect: .zero,
      styleMask: [.borderless, .nonactivatingPanel],
      backing: .buffered,
      defer: false
    )

    self.level = .floating
    self.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
    self.isOpaque = false
    self.backgroundColor = .clear
    self.hasShadow = true
    self.hidesOnDeactivate = false
    self.contentView = contentView

    positionAtBottomRight()
  }

  private func positionAtBottomRight() {
    guard let screen = NSScreen.main else { return }

    let panelWidth: CGFloat = 220
    let panelHeight: CGFloat = 44
    let rightMargin: CGFloat = 20
    let bottomMargin: CGFloat = 20

    let x = screen.frame.maxX - panelWidth - rightMargin
    let y = screen.frame.origin.y + bottomMargin

    setFrame(NSRect(x: x, y: y, width: panelWidth, height: panelHeight), display: true)
  }
}
