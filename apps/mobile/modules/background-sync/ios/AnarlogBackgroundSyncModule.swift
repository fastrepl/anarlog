import ExpoModulesCore
import UIKit
import UserNotifications

public class AnarlogBackgroundSyncModule: Module {
  public func definition() -> ModuleDefinition {
    Name("AnarlogBackgroundSync")

    OnCreate {
      DispatchQueue.main.async {
        BackgroundSyncService.shared.activate()
      }
    }

    AsyncFunction("setEnabled") { (enabled: Bool) in
      BackgroundSyncService.shared.setEnabled(enabled)
    }.runOnQueue(.main)

    AsyncFunction("setPendingWork") { (remaining: Int) in
      BackgroundSyncService.shared.setPendingWork(remaining: remaining)
    }.runOnQueue(.main)

    AsyncFunction("finishBackgroundFlush") {
      BackgroundSyncService.shared.finishBackgroundFlush()
    }.runOnQueue(.main)

    AsyncFunction("notifySyncFailed") {
      BackgroundSyncService.shared.notifySyncFailed()
    }.runOnQueue(.main)
  }
}

// Sync runs silently in background execution time; it never presents
// system UI such as a Live Activity. Only a failed background sync is
// surfaced, as a local notification.
// All state is confined to the main queue.
private final class BackgroundSyncService {
  static let shared = BackgroundSyncService()

  private var observers: [NSObjectProtocol] = []
  private var enabled = false
  private var remaining = 0
  private var backgroundTaskId: UIBackgroundTaskIdentifier = .invalid
  private var flushPending = false
  private let failureNotificationId = "anarlog.sync.failed"

  func activate() {
    guard observers.isEmpty else { return }
    let center = NotificationCenter.default
    observers = [
      center.addObserver(
        forName: UIApplication.didEnterBackgroundNotification,
        object: nil,
        queue: .main
      ) { [weak self] _ in
        self?.appDidEnterBackground()
      },
      center.addObserver(
        forName: UIApplication.didBecomeActiveNotification,
        object: nil,
        queue: .main
      ) { [weak self] _ in
        self?.appDidBecomeActive()
      },
    ]
  }

  func setEnabled(_ enabled: Bool) {
    self.enabled = enabled
    if enabled {
      UNUserNotificationCenter.current().requestAuthorization(
        options: [.alert, .sound]
      ) { _, _ in }
      return
    }
    remaining = 0
    flushPending = false
    endBackgroundTime()
  }

  func setPendingWork(remaining: Int) {
    self.remaining = max(0, remaining)
    if self.remaining == 0, !flushPending { endBackgroundTime() }
  }

  func finishBackgroundFlush() {
    flushPending = false
    if remaining == 0 { endBackgroundTime() }
  }

  func notifySyncFailed() {
    guard enabled, UIApplication.shared.applicationState != .active else {
      return
    }
    let content = UNMutableNotificationContent()
    content.title = "Anarlog couldn't sync"
    content.body = "Open Anarlog to finish syncing your notes."
    content.sound = .default
    UNUserNotificationCenter.current().add(
      UNNotificationRequest(
        identifier: failureNotificationId,
        content: content,
        trigger: nil
      )
    )
  }

  private func appDidEnterBackground() {
    guard enabled else { return }
    flushPending = true
    beginBackgroundTime()
  }

  private func appDidBecomeActive() {
    flushPending = false
    UNUserNotificationCenter.current().removeDeliveredNotifications(
      withIdentifiers: [failureNotificationId]
    )
    endBackgroundTime()
  }

  private func beginBackgroundTime() {
    guard backgroundTaskId == .invalid else { return }
    backgroundTaskId = UIApplication.shared.beginBackgroundTask(
      withName: "AnarlogSync"
    ) { [weak self] in
      self?.endBackgroundTime()
    }
  }

  private func endBackgroundTime() {
    guard backgroundTaskId != .invalid else { return }
    let taskId = backgroundTaskId
    backgroundTaskId = .invalid
    UIApplication.shared.endBackgroundTask(taskId)
  }
}
