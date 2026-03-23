import Cocoa

extension QuitInterceptor {
  func makePanel() -> NSPanel {
    let frame = centeredFrame(size: QuitOverlay.size)

    let panel = NSPanel(
      contentRect: frame,
      styleMask: [.borderless, .nonactivatingPanel],
      backing: .buffered,
      defer: false
    )

    panel.level = .floating
    panel.isFloatingPanel = true
    panel.hidesOnDeactivate = false
    panel.isOpaque = false
    panel.backgroundColor = .clear
    panel.hasShadow = true
    panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .ignoresCycle]
    panel.isMovableByWindowBackground = false
    panel.ignoresMouseEvents = true

    panel.contentView = makeContentView(size: QuitOverlay.size)
    return panel
  }

  func centeredFrame(size: NSSize) -> NSRect {
    guard let screen = NSScreen.main ?? NSScreen.screens.first else {
      return NSRect(origin: .zero, size: size)
    }
    let origin = NSPoint(
      x: screen.frame.midX - size.width / 2,
      y: screen.frame.midY - size.height / 2 + screen.frame.height * QuitOverlay.verticalOffsetRatio
    )
    return NSRect(origin: origin, size: size)
  }

  func makeContentView(size: NSSize) -> NSView {
    let container = NSView(frame: NSRect(origin: .zero, size: size))
    container.wantsLayer = true
    container.layer?.backgroundColor = QuitOverlay.backgroundColor.cgColor
    container.layer?.cornerRadius = QuitOverlay.cornerRadius
    container.layer?.masksToBounds = true

    let messageLabel = makeLabel(QuitOverlay.messageText, color: QuitOverlay.primaryTextColor)
    self.messageLabel = messageLabel

    messageLabel.frame = NSRect(
      x: (size.width - messageLabel.frame.width) / 2,
      y: (size.height - messageLabel.frame.height) / 2,
      width: messageLabel.frame.width,
      height: messageLabel.frame.height
    )

    container.addSubview(messageLabel)

    return container
  }

  func makeLabel(_ text: String, color: NSColor) -> NSTextField {
    let label = NSTextField(labelWithString: text)
    label.font = QuitOverlay.font
    label.textColor = color
    label.alignment = .left
    label.sizeToFit()
    return label
  }

  // MARK: - Panel Visibility

  func showOverlay() {
    if panel == nil {
      panel = makePanel()
    }
    guard let panel else { return }

    panel.alphaValue = 0
    panel.orderFrontRegardless()

    NSAnimationContext.runAnimationGroup { context in
      context.duration = QuitOverlay.animationDuration
      context.timingFunction = CAMediaTimingFunction(name: .easeOut)
      panel.animator().alphaValue = 1.0
    }
  }

  func hidePanel() {
    guard let panel else { return }

    NSAnimationContext.runAnimationGroup({ context in
      context.duration = QuitOverlay.animationDuration
      context.timingFunction = CAMediaTimingFunction(name: .easeIn)
      panel.animator().alphaValue = 0
    }) {
      panel.orderOut(nil)
    }
  }
}
