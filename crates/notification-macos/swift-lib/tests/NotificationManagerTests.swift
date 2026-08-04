import Cocoa
import XCTest

@testable import swift_lib

@_cdecl("rust_on_collapsed_confirm")
func rustOnCollapsedConfirm(_: UnsafePointer<CChar>, _: Int32) {}

@_cdecl("rust_on_expanded_accept")
func rustOnExpandedAccept(_: UnsafePointer<CChar>, _: Int32) {}

@_cdecl("rust_on_dismiss")
func rustOnDismiss(_: UnsafePointer<CChar>, _: Int32) {}

@_cdecl("rust_on_collapsed_timeout")
func rustOnCollapsedTimeout(_: UnsafePointer<CChar>, _: Int32) {}

@_cdecl("rust_on_expanded_start_time_reached")
func rustOnExpandedStartTimeReached(_: UnsafePointer<CChar>, _: Int32) {}

@_cdecl("rust_on_option_selected")
func rustOnOptionSelected(_: UnsafePointer<CChar>, _: Int32) {}

@_cdecl("rust_on_footer_action")
func rustOnFooterAction(_: UnsafePointer<CChar>, _: Int32) {}

final class NotificationManagerTests: XCTestCase {
  private let manager = NotificationManager.shared

  override func setUp() {
    super.setUp()
    resetManager()
  }

  override func tearDown() {
    resetManager()
    super.tearDown()
  }

  func testCapacityEvictionShrinksBookkeepingSynchronously() {
    let notifications = (0..<manager.maxNotifications).map { makeNotification(index: $0) }
    for notification in notifications {
      manager.activeNotifications[notification.key] = notification
      manager.hoverStates[notification.key] = false
    }

    manager.manageNotificationLimit()

    XCTAssertEqual(manager.activeNotifications.count, manager.maxNotifications - 1)
    XCTAssertNil(manager.activeNotifications[notifications[0].key])
  }

  func testRemovalIsIdempotentAndDoesNotRemoveReplacementWithSameKey() {
    let original = makeNotification(index: 0, key: "meeting")
    let replacement = makeNotification(index: 1, key: "meeting")
    manager.activeNotifications[original.key] = replacement
    manager.hoverStates[original.key] = true

    XCTAssertFalse(manager.removeNotification(original))
    XCTAssertTrue(manager.activeNotifications[original.key] === replacement)
    XCTAssertTrue(manager.removeNotification(replacement))
    XCTAssertFalse(manager.removeNotification(replacement))
    XCTAssertNil(manager.activeNotifications[original.key])
    XCTAssertNil(manager.hoverStates[original.key])
  }

  func testReplacingAKeyDismissesTheExistingNotificationBeforeInsertion() {
    let original = makeNotification(index: 0, key: "meeting")
    manager.activeNotifications[original.key] = original
    manager.hoverStates[original.key] = true

    manager.dismissExistingNotification(forKey: original.key)

    XCTAssertNil(manager.activeNotifications[original.key])
    XCTAssertNil(manager.hoverStates[original.key])
  }

  private func makeNotification(index: Int, key: String? = nil) -> NotificationInstance {
    let payload = NotificationPayload(
      key: key ?? "notification-\(index)",
      title: "Title",
      message: "Message",
      timeoutSeconds: 0,
      source: nil,
      startTime: nil,
      participants: nil,
      eventDetails: nil,
      actionLabel: nil,
      actionVariant: nil,
      options: nil,
      footer: nil,
      icon: nil
    )
    return NotificationInstance(
      payload: payload,
      panel: NSPanel(),
      clickableView: ClickableView(frame: .zero),
      creationIndex: index
    )
  }

  private func resetManager() {
    for notification in manager.activeNotifications.values {
      notification.panel.close()
    }
    manager.activeNotifications.removeAll()
    manager.hoverStates.removeAll()
    manager.stopMouseMonitorsIfNeeded()
    manager.stopNativeNotificationMonitorIfNeeded()
  }
}
