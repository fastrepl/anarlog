import Foundation
import SwiftRs

@_cdecl("_show_notification")
public func _showNotification(jsonPayload: SRString) -> Bool {
  let jsonString = jsonPayload.toString()

  guard let data = jsonString.data(using: .utf8),
    let payload = try? JSONDecoder().decode(NotificationPayload.self, from: data)
  else {
    return false
  }

  NotificationManager.shared.show(payload: payload)

  Thread.sleep(forTimeInterval: 0.1)
  return true
}

@_cdecl("_dismiss_all_notifications")
public func _dismissAllNotifications() -> Bool {
  NotificationManager.shared.dismissAll()
  return true
}
