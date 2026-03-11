import Cocoa

class NotificationInstance {
  let payload: NotificationPayload
  let panel: NSPanel
  let clickableView: ClickableView
  let creationIndex: Int
  private var timeoutSeconds: Double = 0

  var key: String { payload.key }

  var isExpanded: Bool = false
  var isAnimating: Bool = false
  var compactContentView: NSView?
  var expandedContentView: NSView?

  var countdownTimer: Timer?
  var meetingStartTime: Date?
  weak var timerLabel: NSTextField?
  weak var progressBar: NotificationBackgroundView? {
    didSet {
      progressBar?.onProgressComplete = { [weak self] in
        self?.dismissWithTimeout()
      }
    }
  }

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
    progressBar?.startProgress(duration: timeoutSeconds)
  }

  func pauseDismissTimer() {
    progressBar?.pauseProgress()
  }

  func resumeDismissTimer() {
    progressBar?.resumeProgress()
  }

  func restartDismissTimer() {
    guard timeoutSeconds > 0 else { return }
    progressBar?.startProgress(duration: timeoutSeconds)
  }

  func dismiss() {
    progressBar?.onProgressComplete = nil
    progressBar?.resetProgress()
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

  deinit {
    progressBar?.onProgressComplete = nil
    countdownTimer?.invalidate()
  }
}
