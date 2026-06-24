import Cocoa

final class FloatingPanelPositionController: NSObject, NSWindowDelegate {
  private var activeScreenId: CGDirectDisplayID?
  private var pinnedOrigin: NSPoint?
  private var isProgrammaticMove = false
  private var programmaticMoveId = 0
  private var programmaticOrigin: NSPoint?
  private let programmaticMoveSuppressionDelay: TimeInterval = 0.15

  func position(
    _ panel: NSPanel,
    force: Bool = false,
    size: NSSize? = nil,
    defaultOrigin: (NSScreen, NSSize) -> NSPoint
  ) {
    let panelSize = size ?? panel.frame.size

    if let pinnedOrigin {
      if force {
        let origin = clampedOrigin(pinnedOrigin, size: panelSize)
        move(panel, to: origin)
        self.pinnedOrigin = origin
        let frame = NSRect(origin: origin, size: panelSize)
        activeScreenId =
          (screen(containing: frame) ?? panel.screen).flatMap { displayId(for: $0) }
      }
      return
    }

    let screen = activeScreen()
    let screenId = displayId(for: screen)
    if !force, screenId == activeScreenId {
      return
    }

    move(panel, to: defaultOrigin(screen, panelSize))
    activeScreenId = screenId
  }

  func setFrame(_ panel: NSPanel, to frame: NSRect, display: Bool, animate: Bool) {
    let wasPinned = pinnedOrigin != nil

    performProgrammaticMove(expectedOrigin: frame.origin) {
      panel.setFrame(frame, display: display, animate: animate)
      return panel.frame.origin
    }

    if wasPinned {
      pinnedOrigin = frame.origin
      activeScreenId =
        (screen(containing: frame) ?? panel.screen).flatMap { displayId(for: $0) }
    }
  }

  func resetActiveScreen() {
    activeScreenId = nil
  }

  func clearPinnedOrigin() {
    pinnedOrigin = nil
  }

  func moveByUserDrag(_ panel: NSPanel, to origin: NSPoint) {
    panel.setFrameOrigin(origin)
    pinnedOrigin = panel.frame.origin
    activeScreenId = panel.screen.flatMap { displayId(for: $0) }
    programmaticOrigin = nil
  }

  func preparePinnedFrameForReplacement(_ panel: NSPanel, size: NSSize) {
    guard pinnedOrigin != nil else { return }

    let frame = NSRect(
      x: panel.frame.minX,
      y: panel.frame.maxY - size.height,
      width: size.width,
      height: size.height)
    pinnedOrigin = frame.origin
    activeScreenId =
      (screen(containing: frame) ?? panel.screen).flatMap { displayId(for: $0) }
  }

  func windowDidMove(_ notification: Notification) {
    guard let panel = notification.object as? NSPanel else { return }
    let origin = panel.frame.origin

    if isProgrammaticMove {
      programmaticOrigin = origin
      return
    }

    if let programmaticOrigin, pointsAreClose(origin, programmaticOrigin) {
      return
    }

    guard isPointerButtonPressed else {
      programmaticOrigin = origin
      return
    }

    programmaticOrigin = nil
    pinnedOrigin = origin
    activeScreenId = panel.screen.flatMap { displayId(for: $0) }
  }

  private func move(_ panel: NSPanel, to origin: NSPoint) {
    performProgrammaticMove(expectedOrigin: origin) {
      panel.setFrameOrigin(origin)
      return panel.frame.origin
    }
  }

  private func performProgrammaticMove(expectedOrigin: NSPoint, updateFrame: () -> NSPoint) {
    programmaticMoveId += 1
    let moveId = programmaticMoveId

    isProgrammaticMove = true
    programmaticOrigin = expectedOrigin
    let actualOrigin = updateFrame()
    programmaticOrigin =
      pointsAreClose(actualOrigin, expectedOrigin) ? actualOrigin : expectedOrigin

    // AppKit can deliver move notifications shortly after programmatic frame changes.
    DispatchQueue.main.asyncAfter(deadline: .now() + programmaticMoveSuppressionDelay) {
      [weak self] in
      guard let self, self.programmaticMoveId == moveId else { return }
      self.isProgrammaticMove = false
    }
  }

  private func activeScreen() -> NSScreen {
    let mouse = NSEvent.mouseLocation
    let screens = NSScreen.screens

    if let exactScreen = screens.first(where: { $0.frame.contains(mouse) }) {
      return exactScreen
    }

    if let activeScreenId,
      let currentScreen = screens.first(where: { displayId(for: $0) == activeScreenId }),
      currentScreen.frame.insetBy(dx: -80, dy: -80).contains(mouse)
    {
      return currentScreen
    }

    return nearestScreen(to: mouse) ?? NSScreen.main ?? screens.first!
  }

  private func clampedOrigin(_ origin: NSPoint, size: NSSize) -> NSPoint {
    let rect = NSRect(origin: origin, size: size)
    guard
      let screen = screen(containing: rect) ?? nearestScreen(to: center(of: rect)) ?? NSScreen.main
    else {
      return origin
    }

    let frame = screen.visibleFrame
    let maxX = max(frame.minX, frame.maxX - size.width)
    let maxY = max(frame.minY, frame.maxY - size.height)

    return NSPoint(
      x: clamped(origin.x, lowerBound: frame.minX, upperBound: maxX),
      y: clamped(origin.y, lowerBound: frame.minY, upperBound: maxY)
    )
  }

  private func screen(containing rect: NSRect) -> NSScreen? {
    let screens = NSScreen.screens
    let center = center(of: rect)
    return screens.first(where: { $0.frame.contains(center) })
      ?? screens.first(where: { $0.frame.intersects(rect) })
  }

  private func nearestScreen(to point: NSPoint) -> NSScreen? {
    let screens = NSScreen.screens
    return screens.min { left, right in
      distanceSquared(from: point, to: left.frame) < distanceSquared(from: point, to: right.frame)
    }
  }

  private func displayId(for screen: NSScreen) -> CGDirectDisplayID? {
    let key = NSDeviceDescriptionKey("NSScreenNumber")
    return (screen.deviceDescription[key] as? NSNumber).map { CGDirectDisplayID($0.uint32Value) }
  }

  private func distanceSquared(from point: NSPoint, to rect: NSRect) -> CGFloat {
    let clampedX = clamped(point.x, lowerBound: rect.minX, upperBound: rect.maxX)
    let clampedY = clamped(point.y, lowerBound: rect.minY, upperBound: rect.maxY)
    let dx = point.x - clampedX
    let dy = point.y - clampedY
    return dx * dx + dy * dy
  }

  private func center(of rect: NSRect) -> NSPoint {
    NSPoint(x: rect.midX, y: rect.midY)
  }

  private func clamped(_ value: CGFloat, lowerBound: CGFloat, upperBound: CGFloat) -> CGFloat {
    min(max(value, lowerBound), upperBound)
  }

  private func pointsAreClose(_ left: NSPoint, _ right: NSPoint) -> Bool {
    abs(left.x - right.x) < 0.5 && abs(left.y - right.y) < 0.5
  }

  private var isPointerButtonPressed: Bool {
    NSEvent.pressedMouseButtons != 0
  }
}
