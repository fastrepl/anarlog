import Cocoa

extension OverlayManager {
  func fadeIn(_ panel: NSPanel) {
    NSAnimationContext.runAnimationGroup { ctx in
      ctx.duration = OverlayTiming.fadeIn
      ctx.timingFunction = CAMediaTimingFunction(name: .easeOut)
      panel.animator().alphaValue = 1
    }
  }

  func fadeOut(_ panel: NSPanel, completion: @escaping () -> Void) {
    NSAnimationContext.runAnimationGroup(
      { ctx in
        ctx.duration = OverlayTiming.fadeOut
        ctx.timingFunction = CAMediaTimingFunction(name: .easeIn)
        panel.animator().alphaValue = 0
      },
      completionHandler: completion
    )
  }
}
