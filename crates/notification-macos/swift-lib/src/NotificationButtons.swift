import Cocoa

protocol TrackableButton: AnyObject {
  var trackingArea: NSTrackingArea? { get set }
}

extension TrackableButton where Self: NSView {
  func setupTrackingArea() {
    if let area = trackingArea { removeTrackingArea(area) }
    let area = NSTrackingArea(
      rect: bounds,
      options: [.activeAlways, .mouseEnteredAndExited, .inVisibleRect],
      owner: self,
      userInfo: nil
    )
    addTrackingArea(area)
    trackingArea = area
  }
}

class CloseButton: NSButton, TrackableButton {
  weak var notification: NotificationInstance?
  var trackingArea: NSTrackingArea?

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
    isBordered = false
    bezelStyle = .regularSquare
    imagePosition = .imageOnly
    imageScaling = .scaleProportionallyDown

    if #available(macOS 11.0, *) {
      let cfg = NSImage.SymbolConfiguration(
        pointSize: CloseButtonConfig.symbolPointSize, weight: .medium)
      image = NSImage(systemSymbolName: "xmark", accessibilityDescription: "Close")?
        .withSymbolConfiguration(cfg)
    } else {
      image = NSImage(named: NSImage.stopProgressTemplateName)
    }
    contentTintColor = NSColor.black.withAlphaComponent(0.6)

    layer?.cornerRadius = CloseButtonConfig.size / 2
    layer?.backgroundColor = NSColor.white.cgColor
    layer?.borderColor = NSColor.black.withAlphaComponent(0.1).cgColor
    layer?.borderWidth = 0.5

    layer?.shadowColor = NSColor.black.cgColor
    layer?.shadowOpacity = 0.2
    layer?.shadowOffset = CGSize(width: 0, height: 1)
    layer?.shadowRadius = 3

    layer?.zPosition = 1000

    alphaValue = 0
    isHidden = true
  }

  override var intrinsicContentSize: NSSize {
    NSSize(width: CloseButtonConfig.size, height: CloseButtonConfig.size)
  }

  override func updateTrackingAreas() {
    super.updateTrackingAreas()
    setupTrackingArea()
  }

  override func mouseDown(with event: NSEvent) {
    layer?.backgroundColor = Colors.closeButtonPressedBg
    DispatchQueue.main.asyncAfter(deadline: .now() + Timing.buttonPress) {
      self.layer?.backgroundColor = NSColor.white.cgColor
    }
    notification?.dismissWithUserAction()
  }

  override func mouseEntered(with event: NSEvent) {
    super.mouseEntered(with: event)
    NSCursor.pointingHand.push()
    layer?.backgroundColor = Colors.closeButtonHoverBg
  }

  override func mouseExited(with event: NSEvent) {
    super.mouseExited(with: event)
    NSCursor.pop()
    layer?.backgroundColor = NSColor.white.cgColor
  }
}

class NotificationButton: NSButton {
  weak var notification: NotificationInstance?

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
    isBordered = false
    bezelStyle = .rounded
    controlSize = .small
    font = NSFont.systemFont(ofSize: Fonts.buttonSize, weight: Fonts.buttonWeight)
    focusRingType = .none

    contentTintColor = NSColor(calibratedWhite: 0.1, alpha: 1.0)
    if #available(macOS 11.0, *) {
      bezelColor = NSColor(calibratedWhite: 0.9, alpha: 1.0)
    }

    layer?.cornerRadius = 8
    layer?.backgroundColor = Colors.buttonNormalBg
    layer?.borderColor = NSColor(calibratedWhite: 0.7, alpha: 0.5).cgColor
    layer?.borderWidth = 0.5

    layer?.shadowColor = NSColor(calibratedWhite: 0.0, alpha: 0.5).cgColor
    layer?.shadowOpacity = 0.2
    layer?.shadowRadius = 2
    layer?.shadowOffset = CGSize(width: 0, height: 1)
  }

  override var intrinsicContentSize: NSSize {
    var s = super.intrinsicContentSize
    s.width += 12
    s.height = max(24, s.height + 2)
    return s
  }

  func animatePress() {
    layer?.backgroundColor = Colors.buttonPressedBg
    DispatchQueue.main.asyncAfter(deadline: .now() + Timing.buttonPress) {
      self.layer?.backgroundColor = Colors.buttonNormalBg
    }
  }

  func performAction() {}

  override func mouseDown(with event: NSEvent) {
    animatePress()
    performAction()
  }
}

class ActionButton: NotificationButton {
  override func performAction() {
    guard let notification = notification else { return }
    RustBridge.onExpandedAccept(key: notification.key)
    notification.dismiss()
  }
}

class DetailsButton: NotificationButton {
  override func performAction() {
    notification?.toggleExpansion()
  }
}

class OptionsButton: NotificationButton {
  var options: [String] = []

  override func performAction() {
    showOptionsMenu()
  }

  func showOptionsMenu() {
    guard notification != nil else { return }

    let menu = NSMenu()
    menu.autoenablesItems = false

    for (index, option) in options.enumerated() {
      let item = NSMenuItem(
        title: option, action: #selector(optionSelected(_:)), keyEquivalent: "")
      item.target = self
      item.tag = index
      item.isEnabled = true
      menu.addItem(item)
    }

    menu.addItem(NSMenuItem.separator())

    let createNewItem = NSMenuItem(
      title: "Create New Note...", action: #selector(optionSelected(_:)), keyEquivalent: "")
    createNewItem.target = self
    createNewItem.tag = options.count
    createNewItem.isEnabled = true
    menu.addItem(createNewItem)

    let location = NSPoint(x: 0, y: bounds.height)
    menu.popUp(positioning: nil, at: location, in: self)
  }

  @objc func optionSelected(_ sender: NSMenuItem) {
    guard let notification = notification else { return }
    RustBridge.onOptionSelected(key: notification.key, selectedIndex: Int32(sender.tag))
    notification.dismiss()
  }
}

class CollapseButton: NSButton, TrackableButton {
  weak var notification: NotificationInstance?
  var trackingArea: NSTrackingArea?

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
    isBordered = false
    bezelStyle = .regularSquare
    imagePosition = .noImage
    title = "Show less"
    font = NSFont.systemFont(ofSize: Fonts.bodySize, weight: Fonts.bodyWeight)
    contentTintColor = NSColor.secondaryLabelColor
    layer?.backgroundColor = NSColor.clear.cgColor
  }

  override var intrinsicContentSize: NSSize {
    var s = super.intrinsicContentSize
    s.height = max(16, s.height)
    return s
  }

  override func updateTrackingAreas() {
    super.updateTrackingAreas()
    setupTrackingArea()
  }

  override func mouseDown(with event: NSEvent) {
    contentTintColor = NSColor.tertiaryLabelColor
    DispatchQueue.main.asyncAfter(deadline: .now() + Timing.buttonPress) {
      self.contentTintColor = NSColor.secondaryLabelColor
    }
    notification?.toggleExpansion()
  }

  override func mouseEntered(with event: NSEvent) {
    super.mouseEntered(with: event)
    NSCursor.pointingHand.push()
    contentTintColor = NSColor.labelColor
  }

  override func mouseExited(with event: NSEvent) {
    super.mouseExited(with: event)
    NSCursor.pop()
    contentTintColor = NSColor.secondaryLabelColor
  }
}
