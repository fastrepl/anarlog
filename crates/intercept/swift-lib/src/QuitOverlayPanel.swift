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

    let pressLabel = makeLabel(QuitOverlay.pressText, color: QuitOverlay.primaryTextColor)
    let holdLabel = makeLabel(QuitOverlay.holdText, color: QuitOverlay.secondaryTextColor)
    self.pressLabel = pressLabel
    self.holdLabel = holdLabel

    let prefixDelta =
      NSAttributedString(
        string: "Press ", attributes: [.font: QuitOverlay.font]
      ).size().width
      - NSAttributedString(
        string: "Hold ", attributes: [.font: QuitOverlay.font]
      ).size().width

    let spacing: CGFloat = 10
    let totalHeight = pressLabel.frame.height + spacing + holdLabel.frame.height
    let topY = (size.height + totalHeight) / 2 - pressLabel.frame.height
    let pressX = (size.width - pressLabel.frame.width) / 2

    pressLabel.frame = NSRect(
      x: pressX,
      y: topY,
      width: pressLabel.frame.width,
      height: pressLabel.frame.height
    )
    holdLabel.frame = NSRect(
      x: pressX + prefixDelta,
      y: topY - spacing - holdLabel.frame.height,
      width: holdLabel.frame.width,
      height: holdLabel.frame.height
    )

    container.addSubview(pressLabel)
    container.addSubview(holdLabel)

    let progress = CALayer()
    progress.anchorPoint = CGPoint(x: 0, y: 0)
    progress.frame = NSRect(
      x: 0,
      y: 0,
      width: 0,
      height: QuitOverlay.progressBarHeight
    )
    progress.backgroundColor = QuitOverlay.progressBarColor.cgColor
    container.layer?.addSublayer(progress)
    progressLayer = progress

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
