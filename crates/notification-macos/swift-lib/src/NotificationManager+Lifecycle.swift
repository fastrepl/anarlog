import Cocoa

extension NotificationManager {
  func createAndShowNotification(payload: NotificationPayload) {
    guard let screen = getTargetScreen() else { return }

    manageNotificationLimit()

    let yPosition = calculateYPosition(screen: screen)
    let panel = createPanel(screen: screen, yPosition: yPosition)
    let clickableView = createClickableView()
    let container = createContainer(clickableView: clickableView)
    let (effectView, backgroundView) = createEffectView(container: container)

    let notification = NotificationInstance(
      payload: payload,
      panel: panel,
      clickableView: clickableView,
      creationIndex: nextCreationIndex
    )
    nextCreationIndex += 1
    clickableView.notification = notification
    notification.progressBar = backgroundView

    if payload.isPersistent {
      backgroundView.isProgressHidden = true
    }

    clickableView.addSubview(container)
    panel.contentView = clickableView
    if isMacOS26() {
      panel.contentView?.wantsLayer = true
      panel.contentView?.layer?.cornerRadius = Layout.cornerRadius
      panel.contentView?.layer?.masksToBounds = true
      if #available(macOS 11.0, *) {
        panel.contentView?.layer?.cornerCurve = .continuous
      }
    }

    setupContent(effectView: effectView, container: container, notification: notification)

    activeNotifications[notification.key] = notification
    hoverStates[notification.key] = false

    showWithAnimation(
      notification: notification, screen: screen, timeoutSeconds: payload.timeoutSeconds)
    ensureGlobalMouseMonitor()
    ensureNativeNotificationMonitor()
  }

  func setupContent(
    effectView: NSVisualEffectView,
    container: NSView,
    notification: NotificationInstance
  ) {
    let contentView = createNotificationView(notification: notification)
    contentView.translatesAutoresizingMaskIntoConstraints = false
    effectView.addSubview(contentView)

    NSLayoutConstraint.activate([
      contentView.leadingAnchor.constraint(
        equalTo: effectView.leadingAnchor, constant: Layout.contentPaddingHorizontal),
      contentView.trailingAnchor.constraint(
        equalTo: effectView.trailingAnchor, constant: -Layout.contentPaddingHorizontal),
      contentView.topAnchor.constraint(
        equalTo: effectView.topAnchor, constant: Layout.contentPaddingVertical),
      contentView.bottomAnchor.constraint(
        equalTo: effectView.bottomAnchor, constant: -Layout.contentPaddingVertical),
    ])

    notification.compactContentView = contentView

    let closeButton = createCloseButton(
      clickableView: notification.clickableView, container: container, notification: notification)
    setupCloseButtonHover(clickableView: notification.clickableView, closeButton: closeButton)
  }
}
