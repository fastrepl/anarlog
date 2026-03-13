import Cocoa

enum QuitOverlay {
  static let size = NSSize(width: 340, height: 96)
  static let cornerRadius: CGFloat = 12
  static let verticalOffsetRatio: CGFloat = 0.15
  static let backgroundColor = NSColor(white: 0.12, alpha: 0.88)

  static let pressText = "Press ⌘Q to Close"
  static let holdText = "Hold ⌘Q to Quit"
  static let font = NSFont.systemFont(ofSize: 22, weight: .medium)
  static let primaryTextColor = NSColor.white
  static let secondaryTextColor = NSColor(white: 1.0, alpha: 0.5)

  static let animationDuration: TimeInterval = 0.15
  static let holdThreshold: TimeInterval = 0.2
  static let holdDuration: TimeInterval = 1.0
  static let overlayDuration: TimeInterval = 1.5

  static let progressBarHeight: CGFloat = 4
  static let progressBarColor = NSColor(white: 1.0, alpha: 0.6)
}
