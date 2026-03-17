import Cocoa

class ShadowContainerView: NSView {
  override func layout() {
    super.layout()
    let pathRect = bounds.insetBy(dx: 0.5, dy: 0.5)
    layer?.shadowPath = CGPath(
      roundedRect: pathRect,
      cornerWidth: Layout.cornerRadius,
      cornerHeight: Layout.cornerRadius,
      transform: nil
    )
  }
}

class NotificationBackgroundView: NSView {
  let bgLayer = CALayer()
  let borderLayer = CALayer()
  let progressLayer = CALayer()

  private var totalDuration: Double = 0
  private var remainingDuration: Double = 0
  private var progressStartTime: Date?
  private var isPaused: Bool = false
  private var progressRatio: CGFloat = 1.0
  var onProgressComplete: (() -> Void)?

  var isProgressHidden: Bool = false {
    didSet {
      progressLayer.isHidden = isProgressHidden
    }
  }

  func makeBackgroundOpaque() {
    bgLayer.backgroundColor = NSColor(calibratedWhite: 0.92, alpha: 1.0).cgColor
    layer?.cornerRadius = Layout.cornerRadius
    bgLayer.cornerRadius = Layout.cornerRadius
    borderLayer.cornerRadius = Layout.cornerRadius
    borderLayer.borderWidth = 1.5
    borderLayer.borderColor = NSColor(calibratedWhite: 0.75, alpha: 0.5).cgColor
    if #available(macOS 11.0, *) {
      layer?.cornerCurve = .continuous
      bgLayer.cornerCurve = .continuous
      borderLayer.cornerCurve = .continuous
    }
  }

  private var progressBarFullWidth: CGFloat {
    bounds.width - (Layout.progressBarInset * 2)
  }

  override init(frame frameRect: NSRect) {
    super.init(frame: frameRect)
    setup()
  }

  required init?(coder: NSCoder) {
    super.init(coder: coder)
    setup()
  }

  private func setup() {
    wantsLayer = true
    layer?.cornerRadius = Layout.cornerRadius
    layer?.masksToBounds = true

    bgLayer.cornerRadius = Layout.cornerRadius
    bgLayer.backgroundColor = Colors.notificationBg
    layer?.addSublayer(bgLayer)

    borderLayer.cornerRadius = Layout.cornerRadius
    borderLayer.borderWidth = 2.0
    borderLayer.borderColor = NSColor.white.cgColor
    layer?.addSublayer(borderLayer)

    progressLayer.backgroundColor = Colors.progressBarBg
    progressLayer.cornerRadius = Layout.progressBarHeight / 2
    progressLayer.anchorPoint = CGPoint(x: 0, y: 0.5)
    layer?.addSublayer(progressLayer)
  }

  override func layout() {
    super.layout()
    CATransaction.begin()
    CATransaction.setDisableActions(true)
    bgLayer.frame = bounds
    borderLayer.frame = bounds
    syncProgressLayerFrame()
    CATransaction.commit()
  }

  private func syncProgressLayerFrame() {
    let width = isPaused ? progressBarFullWidth * progressRatio : progressBarFullWidth
    progressLayer.frame = CGRect(
      x: Layout.progressBarInset,
      y: Layout.progressBarBottomOffset,
      width: width,
      height: Layout.progressBarHeight
    )
  }

  private func runProgressAnimation(from startWidth: CGFloat, duration: Double) {
    CATransaction.begin()
    CATransaction.setDisableActions(true)
    progressLayer.bounds.size.width = startWidth
    CATransaction.commit()

    CATransaction.begin()
    CATransaction.setCompletionBlock { [weak self] in
      guard let self = self, !self.isPaused else { return }
      self.onProgressComplete?()
    }

    let animation = CABasicAnimation(keyPath: "bounds.size.width")
    animation.fromValue = startWidth
    animation.toValue = 0
    animation.duration = duration
    animation.fillMode = .forwards
    animation.isRemovedOnCompletion = false
    animation.timingFunction = CAMediaTimingFunction(name: .linear)

    progressLayer.add(animation, forKey: "progress")
    CATransaction.commit()
  }

  func startProgress(duration: Double) {
    guard duration > 0 else { return }

    totalDuration = duration
    remainingDuration = duration
    progressStartTime = Date()
    isPaused = false
    progressRatio = 1.0

    progressLayer.removeAllAnimations()
    syncProgressLayerFrame()
    runProgressAnimation(from: progressBarFullWidth, duration: duration)
  }

  func pauseProgress() {
    guard !isPaused, let startTime = progressStartTime else { return }
    isPaused = true

    let elapsed = Date().timeIntervalSince(startTime)
    remainingDuration = max(0, totalDuration - elapsed)

    if let presentation = progressLayer.presentation() {
      let currentWidth = presentation.bounds.width
      progressRatio = progressBarFullWidth > 0 ? currentWidth / progressBarFullWidth : 0
    } else {
      progressRatio = totalDuration > 0 ? remainingDuration / totalDuration : 0
    }

    progressLayer.removeAllAnimations()

    CATransaction.begin()
    CATransaction.setDisableActions(true)
    progressLayer.bounds.size.width = progressBarFullWidth * progressRatio
    CATransaction.commit()
  }

  func resumeProgress() {
    guard isPaused, remainingDuration > 0 else { return }
    isPaused = false
    progressStartTime = Date()

    runProgressAnimation(from: progressBarFullWidth * progressRatio, duration: remainingDuration)
  }

  func resetProgress() {
    progressLayer.removeAllAnimations()
    isPaused = false
    progressStartTime = nil
    remainingDuration = 0
    totalDuration = 0
    progressRatio = 1.0

    CATransaction.begin()
    CATransaction.setDisableActions(true)
    progressLayer.bounds.size.width = 0
    CATransaction.commit()
  }
}

class ClickableView: NSView {
  var trackingArea: NSTrackingArea?
  var isHovering = false
  var onHover: ((Bool) -> Void)?
  weak var notification: NotificationInstance?

  override init(frame frameRect: NSRect) {
    super.init(frame: frameRect)
    setupView()
  }

  required init?(coder: NSCoder) {
    super.init(coder: coder)
    setupView()
  }

  private func setupView() {
    wantsLayer = true
    layer?.backgroundColor = NSColor.clear.cgColor
  }

  override func updateTrackingAreas() {
    super.updateTrackingAreas()
    for area in trackingAreas { removeTrackingArea(area) }
    trackingArea = nil

    let options: NSTrackingArea.Options = [
      .activeAlways, .mouseEnteredAndExited, .mouseMoved, .inVisibleRect, .enabledDuringMouseDrag,
    ]

    let area = NSTrackingArea(rect: bounds, options: options, owner: self, userInfo: nil)
    addTrackingArea(area)
    trackingArea = area

    updateHoverStateFromCurrentMouseLocation()
  }

  private func updateHoverStateFromCurrentMouseLocation() {
    guard let win = window else { return }
    let global = win.mouseLocationOutsideOfEventStream
    let local = convert(global, from: nil)
    let inside = bounds.contains(local)
    if inside != isHovering {
      isHovering = inside
      onHover?(inside)
    }
  }

  override func mouseEntered(with event: NSEvent) {
    super.mouseEntered(with: event)
    isHovering = true
    onHover?(true)
  }

  override func mouseExited(with event: NSEvent) {
    super.mouseExited(with: event)
    isHovering = false
    NSCursor.arrow.set()
    onHover?(false)
  }

  override func mouseMoved(with event: NSEvent) {
    super.mouseMoved(with: event)
    let location = convert(event.locationInWindow, from: nil)
    let isInside = bounds.contains(location)
    if isInside != isHovering {
      isHovering = isInside
      onHover?(isInside)
    }
  }

  override func mouseDown(with event: NSEvent) {
    alphaValue = 0.95
    DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { self.alphaValue = 1.0 }
    if let notification = notification {
      if notification.payload.hasOptions, let optionsButton = findOptionsButton(in: self) {
        optionsButton.showOptionsMenu()
      } else {
        RustBridge.onCollapsedConfirm(key: notification.key)
        notification.dismiss()
      }
    }
  }

  private func findOptionsButton(in view: NSView) -> OptionsButton? {
    for subview in view.subviews {
      if let button = subview as? OptionsButton {
        return button
      }
      if let found = findOptionsButton(in: subview) {
        return found
      }
    }
    return nil
  }

  override func viewDidMoveToWindow() {
    super.viewDidMoveToWindow()
    if window != nil { updateTrackingAreas() }
  }
}
