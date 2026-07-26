import Cocoa

extension NotificationManager {
  func ensureGlobalMouseMonitor() {
    guard globalMouseMonitor == nil else { return }

    globalMouseMonitor = NSEvent.addGlobalMonitorForEvents(matching: [
      .mouseMoved, .leftMouseDragged, .rightMouseDragged,
    ]) { [weak self] _ in
      guard let self else { return }
      let pt = NSEvent.mouseLocation
      DispatchQueue.main.async { self.updateHoverForAll(atScreenPoint: pt) }
    }

    localMouseMonitor = NSEvent.addLocalMonitorForEvents(matching: [
      .mouseMoved, .leftMouseDragged, .rightMouseDragged,
    ]) { [weak self] event in
      if let self = self {
        let pt = NSEvent.mouseLocation
        self.updateHoverForAll(atScreenPoint: pt)
      }
      return event
    }
  }

  func stopMouseMonitorsIfNeeded() {
    guard activeNotifications.isEmpty else { return }

    if let monitor = globalMouseMonitor {
      NSEvent.removeMonitor(monitor)
      globalMouseMonitor = nil
    }
    if let monitor = localMouseMonitor {
      NSEvent.removeMonitor(monitor)
      localMouseMonitor = nil
    }
  }

  func updateHoverForAll(atScreenPoint pt: NSPoint) {
    for (key, notif) in activeNotifications {
      let inside = notif.panel.frame.contains(pt)
      let prev = hoverStates[key] ?? false
      if inside != prev {
        hoverStates[key] = inside
        notif.clickableView.onHover?(inside)
      }
    }
  }
}
