import Cocoa

class NotificationManager {
  static let shared = NotificationManager()

  private init() {
    setupDisplayChangeObserver()
  }

  var activeNotifications: [String: NotificationInstance] = [:]
  let maxNotifications = 5
  let notificationSpacing: CGFloat = 10

  var globalMouseMonitor: Any?
  var localMouseMonitor: Any?
  var hoverStates: [String: Bool] = [:]
  var displayChangeObserver: Any?
  var nativeNotificationMonitor: Timer?
  var lastNativeNotificationOffset: CGFloat = 0
  var nextCreationIndex: Int = 0

  func show(payload: NotificationPayload) {
    DispatchQueue.main.async { [weak self] in
      guard let self else { return }
      self.setupApplicationIfNeeded()
      self.createAndShowNotification(payload: payload)
    }
  }

  func dismiss() {
    if let mostRecent = activeNotifications.values.max(by: { $0.creationIndex < $1.creationIndex })
    {
      mostRecent.dismiss()
    }
  }

  func dismissAll() {
    activeNotifications.values.forEach { $0.dismiss() }
  }

  func removeNotification(_ notification: NotificationInstance) {
    activeNotifications.removeValue(forKey: notification.key)
    hoverStates.removeValue(forKey: notification.key)
    repositionNotifications()
    stopMouseMonitorsIfNeeded()
    stopNativeNotificationMonitorIfNeeded()
  }

  func setupApplicationIfNeeded() {
    let app = NSApplication.shared
    if app.delegate == nil {
      app.setActivationPolicy(.accessory)
    }
  }

  func manageNotificationLimit() {
    while activeNotifications.count >= maxNotifications {
      if let oldest = activeNotifications.values.min(by: { $0.creationIndex < $1.creationIndex }) {
        oldest.dismiss()
      }
    }
  }

  deinit {
    if let observer = displayChangeObserver {
      NotificationCenter.default.removeObserver(observer)
    }
    if let monitor = globalMouseMonitor {
      NSEvent.removeMonitor(monitor)
    }
    if let monitor = localMouseMonitor {
      NSEvent.removeMonitor(monitor)
    }
    nativeNotificationMonitor?.invalidate()
  }
}
