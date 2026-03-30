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
  var compactTimerLabel: NSTextField?
  var expandedTimerLabel: NSTextField?
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

  func stopCountdown() {
    countdownTimer?.invalidate()
    countdownTimer = nil
  }

  func setCompactCountdownLabel(_ label: NSTextField) {
    compactTimerLabel = label
    startCountdownIfNeeded()
    updateCountdown()
  }

  func setExpandedCountdownLabel(_ label: NSTextField) {
    expandedTimerLabel = label
    startCountdownIfNeeded()
    updateCountdown()
  }

  func clearExpandedCountdownLabel() {
    expandedTimerLabel = nil
  }

  private func startCountdownIfNeeded() {
    guard meetingStartTime != nil else {
      stopCountdown()
      return
    }
    guard countdownTimer == nil else { return }

    countdownTimer = Timer.scheduledTimer(withTimeInterval: 1.0, repeats: true) { [weak self] _ in
      self?.updateCountdown()
    }
  }

  private func updateCountdown() {
    guard let startTime = meetingStartTime else { return }
    let remaining = startTime.timeIntervalSinceNow

    if remaining <= 0 {
      compactTimerLabel?.stringValue = "Started"
      expandedTimerLabel?.stringValue = "Started"
      countdownTimer?.invalidate()
      countdownTimer = nil

      if isExpanded {
        RustBridge.onExpandedStartTimeReached(key: key)
        dismiss()
      }
    } else {
      let minutes = Int(remaining) / 60
      let seconds = Int(remaining) % 60
      let countdownText = "Begins in \(minutes):\(String(format: "%02d", seconds))"
      compactTimerLabel?.stringValue = countdownText
      expandedTimerLabel?.stringValue = countdownText
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
    compactTimerLabel = nil
    expandedTimerLabel = nil

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
