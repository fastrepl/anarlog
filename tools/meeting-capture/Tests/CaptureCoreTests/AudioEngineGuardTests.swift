import AVFoundation
import AudioEngineGuard
import XCTest

/// AVAudioEngine raises Objective-C exceptions that abort the process. Called raw, each case
/// here fails the test with the exception (XCTest catches it; the recorder could not).
final class AudioEngineGuardTests: XCTestCase {
  /// An engine's input and output nodes reach for audio hardware: the CI runner has none, and a
  /// microphone prompt nobody answers. Either hangs the run, so these run only on a Mac.
  override func setUpWithError() throws {
    try XCTSkipIf(
      ProcessInfo.processInfo.environment["CI"] != nil, "CI runners have no audio devices")
  }

  /// The live crash: a Bluetooth headset left its 44.1 kHz profile between reading the
  /// microphone's format and installing the tap.
  func testMismatchedMicrophoneTapThrows() throws {
    let engine = AVAudioEngine()
    let input = try engine.guardedInputNode()
    let rate = try input.guardedOutputFormat(forBus: 0).sampleRate
    try XCTSkipIf(rate == 0, "This Mac has no input device")
    let stale = AVAudioFormat(
      standardFormatWithSampleRate: rate == 16_000 ? 44_100 : 16_000, channels: 1)!
    do {
      try input.guardedInstallTap(onBus: 0, bufferSize: 1024, format: stale) { _, _ in }
      try input.guardedRemoveTap(onBus: 0)
      // Whether an input refuses another rate depends on the device and its moment.
      throw XCTSkip("This input accepted a \(stale.sampleRate) Hz tap")
    } catch let error as NSError where error.domain == AnarlogAudioEngineErrorDomain {
      XCTAssertTrue(error.localizedDescription.contains("format mismatch"), "\(error)")
    }
    // The failed call left nothing behind: a tap in the right format installs.
    let current = AVAudioFormat(standardFormatWithSampleRate: rate, channels: 1)!
    try input.guardedInstallTap(onBus: 0, bufferSize: 1024, format: current) { _, _ in }
    try input.guardedRemoveTap(onBus: 0)
  }

  func testSecondTapThrows() throws {
    let engine = AVAudioEngine()
    let mixer = engine.mainMixerNode
    try mixer.guardedInstallTap(onBus: 0, bufferSize: 1024, format: nil) { _, _ in }
    XCTAssertThrowsError(
      try mixer.guardedInstallTap(onBus: 0, bufferSize: 1024, format: nil) { _, _ in })
  }

  func testConnectingADetachedNodeThrows() {
    let engine = AVAudioEngine()
    XCTAssertThrowsError(
      try engine.guardedConnect(AVAudioMixerNode(), to: engine.mainMixerNode, format: nil))
  }
}
