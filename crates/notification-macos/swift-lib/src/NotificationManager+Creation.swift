import Cocoa

extension NotificationManager {
  func isMacOS26() -> Bool {
    NSClassFromString("NSGlassEffectView") != nil
  }

  func buttonOverhang() -> CGFloat {
    isMacOS26() ? 0 : Layout.buttonOverhang
  }

  func createPanel(screen: NSScreen? = nil, yPosition: CGFloat, hasFooter: Bool = false) -> NSPanel
  {
    let targetScreen = screen ?? getTargetScreen() ?? NSScreen.main!
    let screenRect = targetScreen.visibleFrame
    let startXPos = screenRect.maxX + Layout.slideInOffset

    let panel = NSPanel(
      contentRect: NSRect(
        x: startXPos, y: yPosition, width: panelWidth(),
        height: panelHeight(hasFooter: hasFooter)),
      styleMask: [.borderless, .nonactivatingPanel],
      backing: .buffered,
      defer: false,
      screen: targetScreen
    )

    panel.level = NSWindow.Level(rawValue: Int(Int32.max))
    panel.isFloatingPanel = true
    panel.hidesOnDeactivate = false
    panel.isOpaque = false
    panel.backgroundColor = .clear
    panel.hasShadow = !isMacOS26()
    panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .ignoresCycle]
    panel.isMovableByWindowBackground = false
    panel.alphaValue = 0

    panel.ignoresMouseEvents = false
    panel.acceptsMouseMovedEvents = true
    return panel
  }

  func createClickableView(hasFooter: Bool = false) -> ClickableView {
    let v = ClickableView(
      frame: NSRect(x: 0, y: 0, width: panelWidth(), height: panelHeight(hasFooter: hasFooter)))
    v.wantsLayer = true
    v.layer?.backgroundColor = NSColor.clear.cgColor
    v.layer?.isOpaque = false
    if isMacOS26() {
      v.layer?.cornerRadius = Layout.cornerRadius
      v.layer?.masksToBounds = true
      if #available(macOS 11.0, *) {
        v.layer?.cornerCurve = .continuous
      }
    }
    v.autoresizingMask = [.width, .height]
    return v
  }

  func createContainer(clickableView: ClickableView) -> NSView {
    let overhang = buttonOverhang()
    let container = ShadowContainerView(
      frame: NSRect(
        x: overhang,
        y: 0,
        width: clickableView.bounds.width - overhang,
        height: clickableView.bounds.height - overhang
      )
    )
    container.wantsLayer = true
    container.layer?.cornerRadius = Layout.cornerRadius
    container.layer?.masksToBounds = false
    if #available(macOS 11.0, *) {
      container.layer?.cornerCurve = .continuous
    }
    container.autoresizingMask = [.width, .height]
    container.layer?.shadowColor = NSColor.black.cgColor
    container.layer?.shadowOpacity = 0.22
    container.layer?.shadowOffset = CGSize(width: 0, height: 2)
    container.layer?.shadowRadius = 12
    return container
  }

  func createEffectView(container: NSView) -> (NSVisualEffectView, NotificationBackgroundView) {
    let isMacOS26 = isMacOS26()

    let effectView = NSVisualEffectView(frame: container.bounds)
    effectView.material = .popover
    effectView.state = .active
    effectView.blendingMode = isMacOS26 ? .withinWindow : .behindWindow
    effectView.wantsLayer = true
    effectView.layer?.cornerRadius = Layout.cornerRadius
    effectView.layer?.masksToBounds = true
    if isMacOS26, #available(macOS 11.0, *) {
      effectView.layer?.cornerCurve = .continuous
    }
    effectView.autoresizingMask = [.width, .height]

    let backgroundView = NotificationBackgroundView(frame: effectView.bounds)
    backgroundView.autoresizingMask = [.width, .height]
    if isMacOS26 {
      backgroundView.makeBackgroundOpaque()
    }
    effectView.addSubview(backgroundView, positioned: .below, relativeTo: nil)

    container.addSubview(effectView)
    return (effectView, backgroundView)
  }

  func defaultNotificationIcon() -> NSImage? {
    if let appIcon = NSApp.applicationIconImage {
      return appIcon
    }
    return NSImage(named: NSImage.applicationIconName)
  }

  func resolveNotificationBundleIcon(_ bundleId: String) -> NSImage? {
    guard let appURL = NSWorkspace.shared.urlForApplication(withBundleIdentifier: bundleId) else {
      return nil
    }
    return NSWorkspace.shared.icon(forFile: appURL.path)
  }

  func resolveNotificationPathIcon(_ path: String) -> NSImage? {
    let expandedPath = NSString(string: path).expandingTildeInPath
    if let image = NSImage(contentsOfFile: expandedPath) {
      return image
    }
    guard FileManager.default.fileExists(atPath: expandedPath) else {
      return nil
    }
    return NSWorkspace.shared.icon(forFile: expandedPath)
  }

  func calendarNotificationIcon() -> NSImage? {
    resolveNotificationBundleIcon("com.apple.iCal")
  }

  func resolveNotificationIconAsset(_ asset: NotificationIconAsset) -> NSImage? {
    switch asset {
    case .appIcon:
      return defaultNotificationIcon()
    case .calendar:
      return calendarNotificationIcon()
    case .bundleId(let bundleId):
      return resolveNotificationBundleIcon(bundleId)
    case .path(let path):
      return resolveNotificationPathIcon(path)
    }
  }

  func composeNotificationOverlayIcon(base: NSImage, badge: NSImage) -> NSImage {
    let width = max(base.size.width, 1)
    let height = max(base.size.height, 1)
    let size = NSSize(width: width, height: height)
    let compositeImage = NSImage(size: size)

    compositeImage.lockFocus()

    base.draw(
      in: NSRect(origin: .zero, size: size),
      from: NSRect(origin: .zero, size: base.size),
      operation: .sourceOver,
      fraction: 1.0
    )

    let badgeSize = min(size.width, size.height) * 0.54
    let badgeInset = min(size.width, size.height) * 0.01
    let borderWidth = badgeSize * 0.1
    let cornerRadius = badgeSize * 0.3
    let badgeRect = NSRect(
      x: size.width - badgeSize - badgeInset,
      y: badgeInset,
      width: badgeSize,
      height: badgeSize
    )

    NSColor.white.setFill()
    let outerPath = NSBezierPath(
      roundedRect: badgeRect, xRadius: cornerRadius, yRadius: cornerRadius)
    outerPath.fill()

    let innerRect = badgeRect.insetBy(dx: borderWidth, dy: borderWidth)
    let innerCornerRadius = max(cornerRadius - borderWidth, borderWidth)
    let innerPath = NSBezierPath(
      roundedRect: innerRect, xRadius: innerCornerRadius, yRadius: innerCornerRadius)
    innerPath.addClip()
    badge.draw(
      in: innerRect,
      from: NSRect(origin: .zero, size: badge.size),
      operation: .sourceOver,
      fraction: 1.0
    )

    compositeImage.unlockFocus()
    return compositeImage
  }

  func resolveNotificationIcon(_ icon: NotificationIcon?) -> NSImage? {
    guard let icon else {
      return defaultNotificationIcon()
    }

    switch icon {
    case .hidden:
      return nil
    case .bundleId(let bundleId):
      return resolveNotificationBundleIcon(bundleId) ?? defaultNotificationIcon()
    case .path(let path):
      return resolveNotificationPathIcon(path) ?? defaultNotificationIcon()
    case .overlay(let base, let badge):
      guard let baseImage = resolveNotificationIconAsset(base) ?? defaultNotificationIcon() else {
        return nil
      }
      guard let badgeImage = resolveNotificationIconAsset(badge) else {
        return baseImage
      }
      return composeNotificationOverlayIcon(base: baseImage, badge: badgeImage)
    }
  }

  func createNotificationIconView(for payload: NotificationPayload) -> NSImageView? {
    guard let image = resolveNotificationIcon(payload.icon) else {
      return nil
    }

    let imageView = NSImageView()
    imageView.image = image
    imageView.imageScaling = .scaleProportionallyUpOrDown
    imageView.translatesAutoresizingMaskIntoConstraints = false
    imageView.wantsLayer = true
    imageView.layer?.shadowColor = NSColor.black.cgColor
    imageView.layer?.shadowOpacity = 0.3
    imageView.layer?.shadowOffset = CGSize(width: 0, height: 1)
    imageView.layer?.shadowRadius = 2
    return imageView
  }

  func createCloseButton(
    clickableView: ClickableView, container: NSView, notification: NotificationInstance
  )
    -> CloseButton
  {
    let closeButton = CloseButton()
    closeButton.notification = notification
    closeButton.translatesAutoresizingMaskIntoConstraints = false
    clickableView.addSubview(closeButton, positioned: .above, relativeTo: nil)

    let buttonOffset =
      isMacOS26() ? (CloseButtonConfig.size / 2) + 4 : (CloseButtonConfig.size / 2) - 2
    NSLayoutConstraint.activate([
      closeButton.centerYAnchor.constraint(equalTo: container.topAnchor, constant: buttonOffset),
      closeButton.centerXAnchor.constraint(
        equalTo: container.leadingAnchor, constant: buttonOffset),
      closeButton.widthAnchor.constraint(equalToConstant: CloseButtonConfig.size),
      closeButton.heightAnchor.constraint(equalToConstant: CloseButtonConfig.size),
    ])
    return closeButton
  }

  func setupCloseButtonHover(clickableView: ClickableView, closeButton: CloseButton) {
    closeButton.alphaValue = 0
    closeButton.isHidden = true

    clickableView.onHover = { [weak clickableView] isHovering in
      if isHovering { closeButton.isHidden = false }
      NSAnimationContext.runAnimationGroup(
        { context in
          context.duration = Timing.hoverFade
          context.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
          closeButton.animator().alphaValue = isHovering ? 1.0 : 0
        },
        completionHandler: {
          if !isHovering { closeButton.isHidden = true }
        }
      )

      if let notification = clickableView?.notification, !notification.isExpanded {
        if isHovering {
          notification.pauseDismissTimer()
        } else {
          notification.resumeDismissTimer()
        }
      }
    }
  }
}
