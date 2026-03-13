import Cocoa

extension NotificationManager {
  func ensureNativeNotificationMonitor() {
    guard nativeNotificationMonitor == nil else { return }
    guard !activeNotifications.isEmpty else { return }

    if let screen = getTargetScreen() {
      lastNativeNotificationOffset = nativeNotificationOccupiedHeight(on: screen)
    } else {
      lastNativeNotificationOffset = 0
    }

    nativeNotificationMonitor = Timer.scheduledTimer(withTimeInterval: 0.75, repeats: true) {
      [weak self] _ in
      guard let self else { return }
      guard !self.activeNotifications.isEmpty else {
        self.stopNativeNotificationMonitorIfNeeded()
        return
      }
      guard let screen = self.getTargetScreen() else { return }

      let currentOffset = self.nativeNotificationOccupiedHeight(on: screen)
      if abs(currentOffset - self.lastNativeNotificationOffset) > 0.5 {
        self.lastNativeNotificationOffset = currentOffset
        self.repositionNotifications(animated: true)
      }
    }
  }

  func stopNativeNotificationMonitorIfNeeded() {
    guard activeNotifications.isEmpty else { return }
    nativeNotificationMonitor?.invalidate()
    nativeNotificationMonitor = nil
    lastNativeNotificationOffset = 0
  }

  func setupDisplayChangeObserver() {
    displayChangeObserver = NotificationCenter.default.addObserver(
      forName: NSApplication.didChangeScreenParametersNotification,
      object: nil,
      queue: .main
    ) { [weak self] _ in
      self?.handleDisplayChange()
    }
  }

  func handleDisplayChange() {
    repositionNotifications(animated: false)
  }

  func getTargetScreen() -> NSScreen? {
    if let menuBarScreen = NSScreen.screens.first(where: { $0.frame.origin == .zero }) {
      return menuBarScreen
    }
    return NSScreen.main ?? NSScreen.screens.first
  }

  func displayID(for screen: NSScreen) -> CGDirectDisplayID? {
    guard
      let number = screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? NSNumber
    else {
      return nil
    }
    return CGDirectDisplayID(number.uint32Value)
  }

  func nativeNotificationOccupiedHeight(on screen: NSScreen) -> CGFloat {
    guard let displayID = displayID(for: screen) else { return 0 }
    let displayBounds = CGDisplayBounds(displayID)
    let scale = max(screen.backingScaleFactor, 1)
    let screenWidthPoints = screen.visibleFrame.width
    let screenHeightPoints = screen.visibleFrame.height
    let maxBannerWidthPoints = min(700, screenWidthPoints * 0.8)
    let maxBannerHeightPoints = min(420, screenHeightPoints * 0.45)
    let maxRightInsetPoints: CGFloat = 180

    guard
      let windowList = CGWindowListCopyWindowInfo(
        [.optionOnScreenOnly, .excludeDesktopElements],
        kCGNullWindowID
      ) as? [[String: Any]]
    else {
      return 0
    }

    var lowestNSBottom: CGFloat?

    for info in windowList {
      guard let ownerName = info[kCGWindowOwnerName as String] as? String,
        ownerName == "NotificationCenter",
        let bounds = info[kCGWindowBounds as String] as? [String: Any],
        let width = (bounds["Width"] as? NSNumber)?.doubleValue,
        let height = (bounds["Height"] as? NSNumber)?.doubleValue,
        width > 0, height > 0
      else {
        continue
      }

      let cgY = (bounds["Y"] as? NSNumber)?.doubleValue ?? 0
      let cgX = (bounds["X"] as? NSNumber)?.doubleValue ?? 0

      let cgRect = CGRect(x: cgX, y: cgY, width: width, height: height)
      guard displayBounds.contains(CGPoint(x: cgRect.midX, y: cgRect.midY)) else {
        continue
      }

      let widthInPoints = width / scale
      let heightInPoints = height / scale
      let rightInsetInPoints = (displayBounds.maxX - cgRect.maxX) / scale
      guard
        widthInPoints <= maxBannerWidthPoints,
        heightInPoints <= maxBannerHeightPoints,
        rightInsetInPoints >= -24,
        rightInsetInPoints <= maxRightInsetPoints
      else {
        continue
      }

      let distanceFromDisplayTopInPoints = (cgRect.maxY - displayBounds.minY) / scale
      let nsBottom = screen.frame.maxY - distanceFromDisplayTopInPoints

      if let current = lowestNSBottom {
        lowestNSBottom = min(current, nsBottom)
      } else {
        lowestNSBottom = nsBottom
      }
    }

    guard let bottom = lowestNSBottom else { return 0 }
    return max(0, screen.visibleFrame.maxY - bottom)
  }

  func repositionNotifications(animated: Bool = true) {
    guard let screen = getTargetScreen() else { return }
    let screenRect = screen.visibleFrame
    let rightPosition = screenRect.maxX - panelWidth() - Layout.rightMargin + buttonOverhang()
    let nativeOffset = nativeNotificationOccupiedHeight(on: screen)

    let sorted = activeNotifications.values.sorted { $0.creationIndex < $1.creationIndex }

    var currentY = screenRect.maxY - Layout.topMargin + buttonOverhang() - nativeOffset

    for notification in sorted {
      let height = notification.panel.frame.height
      currentY -= height
      let newFrame = NSRect(
        x: rightPosition,
        y: currentY,
        width: panelWidth(),
        height: height
      )
      currentY -= notificationSpacing

      if animated {
        NSAnimationContext.runAnimationGroup { context in
          context.duration = Timing.dismiss
          context.timingFunction = CAMediaTimingFunction(name: .easeOut)
          notification.panel.animator().setFrame(newFrame, display: true)
        }
      } else {
        notification.panel.setFrame(newFrame, display: true)
        notification.clickableView.updateTrackingAreas()
        notification.clickableView.window?.invalidateCursorRects(for: notification.clickableView)
        notification.clickableView.window?.resetCursorRects()
      }
    }

    if !animated {
      updateHoverForAll(atScreenPoint: NSEvent.mouseLocation)
    }
  }

  func calculateYPosition(screen: NSScreen? = nil) -> CGFloat {
    guard let targetScreen = screen ?? getTargetScreen() else { return 0 }
    let screenRect = targetScreen.visibleFrame
    let nativeOffset = nativeNotificationOccupiedHeight(on: targetScreen)

    var occupiedHeight: CGFloat = 0
    for notification in activeNotifications.values {
      occupiedHeight += notification.panel.frame.height + notificationSpacing
    }

    let baseY = screenRect.maxY - panelHeight() - Layout.topMargin + buttonOverhang() - nativeOffset
    return baseY - occupiedHeight
  }

  func panelWidth() -> CGFloat {
    Layout.notificationWidth + buttonOverhang()
  }

  func panelHeight(expanded: Bool = false) -> CGFloat {
    let contentHeight = expanded ? Layout.expandedNotificationHeight : Layout.notificationHeight
    return contentHeight + buttonOverhang()
  }
}
