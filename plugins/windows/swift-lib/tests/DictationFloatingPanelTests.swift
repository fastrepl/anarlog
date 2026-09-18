import Cocoa
import XCTest

@testable import swift_lib

final class DictationFloatingPanelTests: XCTestCase {
  func testMicrophoneUpdatesPreservePendingDictationWords() {
    var scheduled: (() -> Void)?
    var result: FloatingBarStatePayload?
    let coalescer = FloatingBarCommandCoalescer(
      scheduler: { scheduled = $0 },
      apply: { action in
        if case .update(let state) = action { result = state }
      })
    let dictation = FloatingDictationPayload(
      sessionId: "recording", phase: "recording", microphone: "Mic", text: "Hello",
      partial: "world", previewEnabled: true, previewUnavailable: false)
    coalescer.enqueueUpdate(
      FloatingBarStatePayload(
        dictation: dictation, amplitude: 0, title: "Dictation", status: .recording,
        colorScheme: .dark, opacity: 1, liveCaptionOpacity: 1, liveCaptionWidth: 360,
        liveCaptionLineCount: 4, liveCaptionPosition: .topCenter, liveCaptionMinimized: false,
        liveCaptionToggleVisible: true, transcriptBubbles: []))
    coalescer.enqueueAmplitude(0.8)
    scheduled?()
    XCTAssertEqual(result?.dictation, dictation)
    XCTAssertEqual(result?.amplitude, 0.8)
  }

  @MainActor
  func testDictationAppliesSharedSettingsWithoutChangingMeetingMinimizedState() {
    let settings = FloatingOverlaySettingsModel.shared
    let saved = (
      settings.floatingBarOpacity, settings.liveCaptionOpacity,
      settings.liveCaptionWidth, settings.liveCaptionLineCount,
      settings.liveCaptionPosition, settings.liveCaptionMinimized
    )
    defer {
      (
        settings.floatingBarOpacity, settings.liveCaptionOpacity,
        settings.liveCaptionWidth, settings.liveCaptionLineCount,
        settings.liveCaptionPosition, settings.liveCaptionMinimized
      ) = saved
    }
    settings.apply(
      floatingBarState: FloatingBarStatePayload(
        dictation: FloatingDictationPayload(
          sessionId: "dictation", phase: "recording", microphone: "Mic", text: "",
          partial: "", previewEnabled: true, previewUnavailable: false),
        amplitude: 0, title: "Dictation", status: .recording, colorScheme: .dark,
        opacity: 0.4, liveCaptionOpacity: 0.6, liveCaptionWidth: 400,
        liveCaptionLineCount: 2, liveCaptionPosition: .topCenter,
        liveCaptionMinimized: !saved.5, liveCaptionToggleVisible: true, transcriptBubbles: []))
    XCTAssertEqual(settings.floatingBarOpacity, 0.4)
    XCTAssertEqual(settings.liveCaptionOpacity, 0.6)
    XCTAssertEqual(settings.liveCaptionWidth, 400)
    XCTAssertEqual(settings.liveCaptionLineCount, 2)
    XCTAssertEqual(settings.liveCaptionMinimized, saved.5)
  }

  @MainActor
  func testDictationCannotTakeKeyboardFocusFromTheDestination() {
    _ = NSApplication.shared
    let panel = FloatingBarPanel(
      contentRect: .zero, styleMask: [.borderless, .nonactivatingPanel, .resizable],
      backing: .buffered, defer: false)
    panel.dictationMode = true
    XCTAssertFalse(panel.canBecomeKey)
    XCTAssertFalse(panel.canBecomeMain)
  }

  func testDictationFitsTheSharedPanelLayout() {
    XCTAssertEqual(
      FloatingBarLayout.containerSize(isExpanded: false, showsExpand: true),
      NSSize(width: 108, height: 67))
    XCTAssertEqual(
      FloatingBarLayout.containerSize(isExpanded: true, showsExpand: true),
      NSSize(width: 368, height: 459))
  }
}
