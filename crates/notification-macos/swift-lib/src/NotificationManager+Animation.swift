import Cocoa

extension NotificationManager {
  func showWithAnimation(
    notification: NotificationInstance, screen: NSScreen, timeoutSeconds: Double
  ) {
    let screenRect = screen.visibleFrame
    let finalX = screenRect.maxX - panelWidth() - Layout.rightMargin + buttonOverhang()
    let y = notification.panel.frame.minY

    notification.panel.setFrame(
      NSRect(
        x: screenRect.maxX + Layout.slideInOffset,
        y: y,
        width: panelWidth(),
        height: panelHeight()
      ),
      display: false
    )

    notification.panel.orderFrontRegardless()
    notification.panel.makeKeyAndOrderFront(nil)

    animate(duration: Timing.slideIn, timing: .easeOut) {
      notification.panel.animator().setFrame(
        NSRect(x: finalX, y: y, width: self.panelWidth(), height: self.panelHeight()),
        display: true
      )
      notification.panel.animator().alphaValue = 1.0
    } completion: {
      self.refreshTrackingAreas(for: notification)
      self.updateHoverForAll(atScreenPoint: NSEvent.mouseLocation)
      if timeoutSeconds > 0 {
        notification.startDismissTimer(timeoutSeconds: timeoutSeconds)
      }
    }
  }

  func animateExpansion(notification: NotificationInstance, isExpanded: Bool) {
    let currentFrame = notification.panel.frame
    let targetHeight = panelHeight(expanded: isExpanded)
    let newFrame = NSRect(
      x: currentFrame.minX,
      y: currentFrame.minY - (targetHeight - currentFrame.height),
      width: currentFrame.width,
      height: targetHeight
    )

    guard let effectView = findEffectView(in: notification) else {
      notification.isAnimating = false
      return
    }

    if isExpanded {
      animateToExpanded(notification: notification, effectView: effectView, frame: newFrame)
    } else {
      animateToCompact(notification: notification, frame: newFrame)
    }
  }

  private func animateToExpanded(
    notification: NotificationInstance, effectView: NSVisualEffectView, frame: NSRect
  ) {
    notification.pauseDismissTimer()
    notification.compactContentView?.isHidden = true

    let expandedView = createExpandedNotificationView(notification: notification)
    expandedView.translatesAutoresizingMaskIntoConstraints = false
    expandedView.alphaValue = 0
    notification.expandedContentView = expandedView

    animate(duration: Timing.expansion, timing: .easeInEaseOut) {
      notification.panel.animator().setFrame(frame, display: true)
    } completion: {
      effectView.addSubview(expandedView)
      self.pinToEdges(expandedView, in: effectView)

      self.animate(duration: Timing.fadeIn, timing: .easeOut) {
        expandedView.animator().alphaValue = 1.0
      } completion: {
        self.finishExpansionAnimation(notification)
      }
    }
  }

  private func animateToCompact(notification: NotificationInstance, frame: NSRect) {
    notification.stopCountdown()
    notification.expandedContentView?.removeFromSuperview()
    notification.expandedContentView = nil
    notification.compactContentView?.alphaValue = 0
    notification.compactContentView?.isHidden = false

    animate(duration: Timing.expansion, timing: .easeInEaseOut) {
      notification.panel.animator().setFrame(frame, display: true)
      notification.compactContentView?.animator().alphaValue = 1.0
    } completion: {
      if !notification.clickableView.isHovering {
        notification.resumeDismissTimer()
      }
      self.finishExpansionAnimation(notification)
    }
  }

  private func finishExpansionAnimation(_ notification: NotificationInstance) {
    notification.isAnimating = false
    notification.clickableView.updateTrackingAreas()
    repositionNotifications()
  }

  private func findEffectView(in notification: NotificationInstance) -> NSVisualEffectView? {
    notification.clickableView.subviews.first?.subviews.first as? NSVisualEffectView
  }

  private func pinToEdges(_ view: NSView, in container: NSView) {
    NSLayoutConstraint.activate([
      view.leadingAnchor.constraint(
        equalTo: container.leadingAnchor, constant: Layout.expandedPaddingHorizontal),
      view.trailingAnchor.constraint(
        equalTo: container.trailingAnchor, constant: -Layout.expandedPaddingHorizontal),
      view.topAnchor.constraint(
        equalTo: container.topAnchor, constant: Layout.expandedPaddingVertical),
      view.bottomAnchor.constraint(
        equalTo: container.bottomAnchor, constant: -Layout.expandedPaddingVertical),
    ])
  }

  private func refreshTrackingAreas(for notification: NotificationInstance) {
    DispatchQueue.main.async {
      notification.clickableView.updateTrackingAreas()
      notification.clickableView.window?.invalidateCursorRects(for: notification.clickableView)
      notification.clickableView.window?.resetCursorRects()
    }
  }

  func animate(
    duration: TimeInterval,
    timing: CAMediaTimingFunctionName,
    animations: @escaping () -> Void,
    completion: @escaping () -> Void
  ) {
    NSAnimationContext.runAnimationGroup(
      { context in
        context.duration = duration
        context.timingFunction = CAMediaTimingFunction(name: timing)
        animations()
      }, completionHandler: completion)
  }
}
