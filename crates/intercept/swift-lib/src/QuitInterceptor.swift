import Cocoa

final class QuitInterceptor {
  static let shared = QuitInterceptor()

  enum State {
    case idle
    case firstPress
    case awaiting
    case holding
  }

  var keyMonitor: Any?
  var panel: NSPanel?
  var progressLayer: CALayer?
  var pressLabel: NSTextField?
  var holdLabel: NSTextField?
  var state: State = .idle
  var dismissTimer: DispatchWorkItem?
  var holdThresholdTimer: DispatchWorkItem?
  var quitTimer: DispatchWorkItem?

  // MARK: - Setup

  func setup() {
    keyMonitor = NSEvent.addLocalMonitorForEvents(matching: [.keyDown, .keyUp, .flagsChanged]) {
      [weak self] event in
      guard let self else { return event }

      switch event.type {
      case .keyDown:
        return self.handleKeyDown(event)
      case .keyUp:
        self.handleKeyUp(event)
        return event
      case .flagsChanged:
        self.handleFlagsChanged(event)
        return event
      default:
        return event
      }
    }
  }

  // MARK: - Actions

  func performQuit() {
    resetProgress()
    setDefaultAppearance()
    rustSetForceQuit()
    hidePanel()
    DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) {
      NSApplication.shared.terminate(nil)
    }
  }

  func performClose() {
    hidePanel()
    rustPerformClose()
  }

  // MARK: - State Machine

  func onCmdQPressed() {
    switch state {
    case .idle:
      state = .firstPress
      showOverlay()
      scheduleTimer(&holdThresholdTimer, delay: QuitOverlay.holdThreshold) { [weak self] in
        guard let self, self.state == .firstPress else { return }
        self.state = .holding
        self.setHoldingAppearance()
        self.startProgressAnimation()
        self.scheduleTimer(&self.quitTimer, delay: QuitOverlay.holdDuration) { [weak self] in
          self?.performQuit()
        }
      }

    case .firstPress, .holding:
      break

    case .awaiting:
      state = .idle
      cancelTimer(&dismissTimer)
      performClose()
    }
  }

  func onKeyReleased() {
    switch state {
    case .idle, .awaiting:
      break

    case .firstPress:
      state = .awaiting
      cancelTimer(&holdThresholdTimer)
      scheduleTimer(&dismissTimer, delay: QuitOverlay.overlayDuration) { [weak self] in
        guard let self, self.state == .awaiting else { return }
        self.state = .idle
        self.hidePanel()
      }

    case .holding:
      state = .awaiting
      cancelTimer(&quitTimer)
      resetProgress()
      setDefaultAppearance()
      scheduleTimer(&dismissTimer, delay: QuitOverlay.overlayDuration) { [weak self] in
        guard let self, self.state == .awaiting else { return }
        self.state = .idle
        self.hidePanel()
      }
    }
  }

  // MARK: - Label Emphasis

  func setHoldingAppearance() {
    pressLabel?.textColor = QuitOverlay.secondaryTextColor
    holdLabel?.textColor = QuitOverlay.primaryTextColor
  }

  func setDefaultAppearance() {
    pressLabel?.textColor = QuitOverlay.primaryTextColor
    holdLabel?.textColor = QuitOverlay.secondaryTextColor
  }

  // MARK: - Progress Bar

  func startProgressAnimation() {
    guard let progressLayer else { return }

    progressLayer.removeAllAnimations()

    let animation = CABasicAnimation(keyPath: "bounds.size.width")
    animation.fromValue = 0
    animation.toValue = QuitOverlay.size.width
    animation.duration = QuitOverlay.holdDuration
    animation.timingFunction = CAMediaTimingFunction(name: .linear)
    animation.fillMode = .forwards
    animation.isRemovedOnCompletion = false

    progressLayer.add(animation, forKey: "progress")
  }

  func resetProgress() {
    guard let progressLayer else { return }
    progressLayer.removeAllAnimations()
    progressLayer.frame.size.width = 0
  }

  // MARK: - Timer Helpers

  func scheduleTimer(
    _ timer: inout DispatchWorkItem?, delay: TimeInterval, action: @escaping () -> Void
  ) {
    timer?.cancel()
    let workItem = DispatchWorkItem(block: action)
    timer = workItem
    DispatchQueue.main.asyncAfter(deadline: .now() + delay, execute: workItem)
  }

  func cancelTimer(_ timer: inout DispatchWorkItem?) {
    timer?.cancel()
    timer = nil
  }

  // MARK: - Event Handlers

  func handleKeyDown(_ event: NSEvent) -> NSEvent? {
    let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
    let isQ = event.charactersIgnoringModifiers?.lowercased() == "q"
    guard flags.contains(.command), isQ else { return event }

    if flags.contains(.shift) {
      performQuit()
      return nil
    }

    if event.isARepeat { return nil }
    onCmdQPressed()
    return nil
  }

  func handleKeyUp(_ event: NSEvent) {
    if event.charactersIgnoringModifiers?.lowercased() == "q" {
      onKeyReleased()
    }
  }

  func handleFlagsChanged(_ event: NSEvent) {
    let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
    if !flags.contains(.command) {
      onKeyReleased()
    }
  }
}
