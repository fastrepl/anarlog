import Cocoa

class ShadowContainerView: NSView {
  override func layout() {
    super.layout()
    let pathRect = bounds.insetBy(dx: 0.5, dy: 0.5)
    layer?.shadowPath = CGPath(
      roundedRect: pathRect,
      cornerWidth: Layout.cornerRadius,
      cornerHeight: Layout.cornerRadius,
      transform: nil
    )
  }
}

class NotificationBackgroundView: NSView {
  let bgLayer = CALayer()
  let borderLayer = CALayer()

  func makeBackgroundOpaque() {
    bgLayer.backgroundColor = NSColor(calibratedWhite: 0.92, alpha: 1.0).cgColor
    layer?.cornerRadius = Layout.cornerRadius
    bgLayer.cornerRadius = Layout.cornerRadius
    borderLayer.cornerRadius = Layout.cornerRadius
    borderLayer.borderWidth = 1.5
    borderLayer.borderColor = NSColor(calibratedWhite: 0.75, alpha: 0.5).cgColor
    if #available(macOS 11.0, *) {
      layer?.cornerCurve = .continuous
      bgLayer.cornerCurve = .continuous
      borderLayer.cornerCurve = .continuous
    }
  }

  override init(frame frameRect: NSRect) {
    super.init(frame: frameRect)
    setup()
  }

  required init?(coder: NSCoder) {
    super.init(coder: coder)
    setup()
  }

  private func setup() {
    wantsLayer = true
    layer?.cornerRadius = Layout.cornerRadius
    layer?.masksToBounds = true

    bgLayer.cornerRadius = Layout.cornerRadius
    bgLayer.backgroundColor = Colors.notificationBg
    layer?.addSublayer(bgLayer)

    borderLayer.cornerRadius = Layout.cornerRadius
    borderLayer.borderWidth = 2.0
    borderLayer.borderColor = NSColor.white.cgColor
    layer?.addSublayer(borderLayer)
  }

  override func layout() {
    super.layout()
    CATransaction.begin()
    CATransaction.setDisableActions(true)
    bgLayer.frame = bounds
    borderLayer.frame = bounds
    CATransaction.commit()
  }
}

class ClickableView: NSView {
  var trackingArea: NSTrackingArea?
  var isHovering = false
  var onHover: ((Bool) -> Void)?
  weak var notification: NotificationInstance?

  override init(frame frameRect: NSRect) {
    super.init(frame: frameRect)
    setupView()
  }

  required init?(coder: NSCoder) {
    super.init(coder: coder)
    setupView()
  }

  private func setupView() {
    wantsLayer = true
    layer?.backgroundColor = NSColor.clear.cgColor
  }

  override func updateTrackingAreas() {
    super.updateTrackingAreas()
    for area in trackingAreas { removeTrackingArea(area) }
    trackingArea = nil

    let options: NSTrackingArea.Options = [
      .activeAlways, .mouseEnteredAndExited, .mouseMoved, .inVisibleRect, .enabledDuringMouseDrag,
    ]

    let area = NSTrackingArea(rect: bounds, options: options, owner: self, userInfo: nil)
    addTrackingArea(area)
    trackingArea = area

    updateHoverStateFromCurrentMouseLocation()
  }

  private func updateHoverStateFromCurrentMouseLocation() {
    guard let win = window else { return }
    let global = win.mouseLocationOutsideOfEventStream
    let local = convert(global, from: nil)
    let inside = bounds.contains(local)
    if inside != isHovering {
      isHovering = inside
      onHover?(inside)
    }
  }

  override func mouseEntered(with event: NSEvent) {
    super.mouseEntered(with: event)
    isHovering = true
    onHover?(true)
  }

  override func mouseExited(with event: NSEvent) {
    super.mouseExited(with: event)
    isHovering = false
    NSCursor.arrow.set()
    onHover?(false)
  }

  override func mouseMoved(with event: NSEvent) {
    super.mouseMoved(with: event)
    let location = convert(event.locationInWindow, from: nil)
    let isInside = bounds.contains(location)
    if isInside != isHovering {
      isHovering = isInside
      onHover?(isInside)
    }
  }

  override func mouseDown(with event: NSEvent) {
    alphaValue = 0.95
    DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { self.alphaValue = 1.0 }
    if let notification = notification {
      if notification.payload.hasOptions, let optionsButton = findOptionsButton(in: self) {
        optionsButton.showOptionsMenu()
      } else if notification.payload.hasExpandableContent {
        notification.toggleExpansion()
      } else {
        RustBridge.onCollapsedConfirm(key: notification.key)
        notification.dismiss()
      }
    }
  }

  private func findOptionsButton(in view: NSView) -> OptionsButton? {
    for subview in view.subviews {
      if let button = subview as? OptionsButton {
        return button
      }
      if let found = findOptionsButton(in: subview) {
        return found
      }
    }
    return nil
  }

  override func viewDidMoveToWindow() {
    super.viewDidMoveToWindow()
    if window != nil { updateTrackingAreas() }
  }
}
