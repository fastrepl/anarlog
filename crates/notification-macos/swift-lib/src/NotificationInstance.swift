import Cocoa

class NotificationInstance {
  let payload: NotificationPayload
  let panel: NSPanel
  let clickableView: ClickableView
  let creationIndex: Int
  private var timeoutSeconds: Double = 0
  private var remainingDismissSeconds: Double = 0
  private var dismissStartTime: Date?

  var key: String { payload.key }

  var isExpanded: Bool = false
  var isAnimating: Bool = false
  var compactContentView: NSView?
  var expandedContentView: NSView?
  weak var effectView: NSVisualEffectView?
  weak var compactActionButton: CompactActionButton?

  var countdownTimer: Timer?
  var dismissTimer: Timer?
  var meetingStartTime: Date?
  weak var timerLabel: NSTextField?

  init(
    payload: NotificationPayload, panel: NSPanel, clickableView: ClickableView, creationIndex: Int
  ) {
    self.payload = payload
    self.panel = panel
    self.clickableView = clickableView
    self.creationIndex = creationIndex

    if let startTime = payload.startTime, startTime > 0 {
      self.meetingStartTime = Date(timeIntervalSince1970: TimeInterval(startTime))
    }
  }

  func toggleExpansion() {
    guard !isAnimating else { return }
    isAnimating = true
    isExpanded.toggle()
    NotificationManager.shared.animateExpansion(notification: self, isExpanded: isExpanded)
  }

  func startCountdown(label: NSTextField) {
    timerLabel = label
    updateCountdown()

    countdownTimer?.invalidate()
    countdownTimer = Timer.scheduledTimer(withTimeInterval: 1.0, repeats: true) { [weak self] _ in
      self?.updateCountdown()
    }
  }

  func stopCountdown() {
    countdownTimer?.invalidate()
    countdownTimer = nil
    timerLabel = nil
  }

  private func updateCountdown() {
    guard let startTime = meetingStartTime, let label = timerLabel else { return }
    let remaining = startTime.timeIntervalSinceNow

    if remaining <= 0 {
      label.stringValue = "Started"
      countdownTimer?.invalidate()
      countdownTimer = nil

      if isExpanded {
        RustBridge.onExpandedStartTimeReached(key: key)
        dismiss()
      }
    } else {
      let minutes = Int(remaining) / 60
      let seconds = Int(remaining) % 60
      label.stringValue = "Begins in \(minutes):\(String(format: "%02d", seconds))"
    }
  }

  func startDismissTimer(timeoutSeconds: Double) {
    self.timeoutSeconds = timeoutSeconds
    remainingDismissSeconds = timeoutSeconds
    dismissStartTime = Date()
    scheduleDismissTimer(after: timeoutSeconds)

    if let compactActionButton {
      compactActionButton.startProgress(duration: timeoutSeconds)
    }
  }

  func pauseDismissTimer() {
    guard timeoutSeconds > 0 else { return }
    if let dismissStartTime {
      let elapsed = Date().timeIntervalSince(dismissStartTime)
      remainingDismissSeconds = max(0, remainingDismissSeconds - elapsed)
      self.dismissStartTime = nil
    }
    dismissTimer?.invalidate()
    dismissTimer = nil

    if let compactActionButton {
      compactActionButton.pauseProgress()
    }
  }

  func resumeDismissTimer() {
    guard timeoutSeconds > 0, remainingDismissSeconds > 0 else { return }
    dismissStartTime = Date()
    scheduleDismissTimer(after: remainingDismissSeconds)

    if let compactActionButton {
      compactActionButton.resumeProgress()
    }
  }

  func restartDismissTimer() {
    guard timeoutSeconds > 0 else { return }
    dismissTimer?.invalidate()
    dismissTimer = nil
    remainingDismissSeconds = timeoutSeconds
    dismissStartTime = Date()
    scheduleDismissTimer(after: timeoutSeconds)

    if let compactActionButton {
      compactActionButton.startProgress(duration: timeoutSeconds)
    }
  }

  func dismiss() {
    dismissTimer?.invalidate()
    dismissTimer = nil
    dismissStartTime = nil
    remainingDismissSeconds = 0
    compactActionButton?.resetProgress()
    stopCountdown()

    NSAnimationContext.runAnimationGroup({ context in
      context.duration = Timing.dismiss
      context.timingFunction = CAMediaTimingFunction(name: .easeIn)
      self.panel.animator().alphaValue = 0
    }) {
      self.panel.close()
      NotificationManager.shared.removeNotification(self)
    }
  }

  func dismissWithUserAction() {
    RustBridge.onDismiss(key: key)
    dismiss()
  }

  func dismissWithTimeout() {
    RustBridge.onCollapsedTimeout(key: key)
    dismiss()
  }

  private func scheduleDismissTimer(after duration: Double) {
    guard duration > 0 else { return }
    dismissTimer?.invalidate()
    dismissTimer = Timer.scheduledTimer(withTimeInterval: duration, repeats: false) {
      [weak self] _ in
      self?.dismissWithTimeout()
    }
  }

  deinit {
    countdownTimer?.invalidate()
    dismissTimer?.invalidate()
  }
}
