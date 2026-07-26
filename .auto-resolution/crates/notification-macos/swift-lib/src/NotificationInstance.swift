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
  weak var compactMessageLabel: NSTextField?
  weak var stopCountdownLabel: NSTextField?
  var stopCountdownTimer: Timer?

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

  func bindCompactMessageLabel(_ label: NSTextField) {
    compactMessageLabel = label
    updateScheduleLabels()
    startScheduleUpdates()
  }

  func bindExpandedTimerLabel(_ label: NSTextField) {
    timerLabel = label
    updateScheduleLabels()
    startScheduleUpdates()
  }

  func clearExpandedTimerLabel() {
    timerLabel = nil
  }

  func bindStopCountdownLabel(_ label: NSTextField) {
    stopCountdownLabel = label
    updateStopCountdownLabel(remainingSeconds: payload.timeoutSeconds)
  }

  func startScheduleUpdates() {
    guard let meetingStartTime else { return }
    updateScheduleLabels()

    guard meetingStartTime.timeIntervalSinceNow > 0 else { return }
    guard countdownTimer == nil else { return }
    countdownTimer?.invalidate()
    countdownTimer = Timer.scheduledTimer(withTimeInterval: 1.0, repeats: true) { [weak self] _ in
      self?.updateScheduleLabels()
    }
  }

  func stopScheduleUpdates() {
    countdownTimer?.invalidate()
    countdownTimer = nil
    stopCountdownTimer?.invalidate()
    stopCountdownTimer = nil
    timerLabel = nil
    compactMessageLabel = nil
    stopCountdownLabel = nil
  }

  private func updateScheduleLabels() {
    guard let startTime = meetingStartTime else { return }
    let remaining = startTime.timeIntervalSinceNow

    if remaining <= 0 {
      compactMessageLabel?.stringValue = "Started"
      timerLabel?.stringValue = "Started"
      countdownTimer?.invalidate()
      countdownTimer = nil
    } else {
      compactMessageLabel?.stringValue = compactScheduleText(remaining)
      timerLabel?.stringValue = expandedScheduleText(remaining)
    }
  }

  private func compactScheduleText(_ remaining: TimeInterval) -> String {
    let minutes = max(1, Int(ceil(remaining / 60)))
    return "Starting in \(minutes) minute\(minutes == 1 ? "" : "s")"
  }

  private func expandedScheduleText(_ remaining: TimeInterval) -> String {
    let minutes = Int(remaining) / 60
    let seconds = Int(remaining) % 60
    return "Begins in \(minutes):\(String(format: "%02d", seconds))"
  }

  func startDismissTimer(timeoutSeconds: Double) {
    self.timeoutSeconds = timeoutSeconds
    remainingDismissSeconds = timeoutSeconds
    dismissStartTime = Date()
    scheduleDismissTimer(after: timeoutSeconds)
    startStopCountdownUpdates()

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
    stopCountdownTimer?.invalidate()
    stopCountdownTimer = nil
    updateStopCountdownLabel(remainingSeconds: remainingDismissSeconds)

    if let compactActionButton {
      compactActionButton.pauseProgress()
    }
  }

  func resumeDismissTimer() {
    guard timeoutSeconds > 0, remainingDismissSeconds > 0 else { return }
    dismissStartTime = Date()
    scheduleDismissTimer(after: remainingDismissSeconds)
    startStopCountdownUpdates()

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
    startStopCountdownUpdates()

    if let compactActionButton {
      compactActionButton.startProgress(duration: timeoutSeconds)
    }
  }

  func dismiss() {
    dismissTimer?.invalidate()
    dismissTimer = nil
    stopCountdownTimer?.invalidate()
    stopCountdownTimer = nil
    dismissStartTime = nil
    remainingDismissSeconds = 0
    compactActionButton?.resetProgress()
    stopScheduleUpdates()

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

  func stopCountdownText(_ remainingSeconds: Double) -> String {
    let seconds = max(0, Int(ceil(remainingSeconds)))
    return "Anarlog will stop listening in \(seconds) seconds."
  }

  private func startStopCountdownUpdates() {
    guard stopCountdownLabel != nil else { return }
    updateStopCountdownLabel()

    stopCountdownTimer?.invalidate()
    stopCountdownTimer = Timer.scheduledTimer(withTimeInterval: 1.0, repeats: true) {
      [weak self] _ in
      self?.updateStopCountdownLabel()
    }
  }

  private func updateStopCountdownLabel(remainingSeconds: Double? = nil) {
    guard stopCountdownLabel != nil else { return }

    let remaining: Double
    if let remainingSeconds {
      remaining = remainingSeconds
    } else if let dismissStartTime {
      remaining = max(0, remainingDismissSeconds - Date().timeIntervalSince(dismissStartTime))
    } else {
      remaining = remainingDismissSeconds
    }

    stopCountdownLabel?.stringValue = stopCountdownText(remaining)
  }

  deinit {
    countdownTimer?.invalidate()
    dismissTimer?.invalidate()
    stopCountdownTimer?.invalidate()
  }
}
