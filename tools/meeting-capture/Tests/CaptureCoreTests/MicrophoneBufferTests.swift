import AVFoundation
import XCTest

@testable import CaptureCore

final class MicrophoneBufferTests: XCTestCase {
  func testTapBufferIsOwnedBeforeTheEngineReusesIt() throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: false)
    defer { try? FileManager.default.removeItem(at: directory) }
    let output = try AudioOutput(
      directory: directory, captureID: "capture", processID: 0, bundleID: "",
      microphoneID: nil)
    let format = AVAudioFormat(standardFormatWithSampleRate: 48_000, channels: 1)!
    let pcm = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: 480)!
    pcm.frameLength = 480
    pcm.floatChannelData![0].initialize(repeating: 0.25, count: 480)
    output.appendMicrophone(pcm, hostTime: output.origin)
    pcm.floatChannelData![0].update(repeating: 0.9, count: 480)
    try output.end()
    let result = try output.finish(reason: "test")
    XCTAssertEqual(result.value("microphoneFrames"), "480")
    XCTAssertEqual(result.value("remoteFrames"), "0")
    let file = try AVAudioFile(forReading: directory.appendingPathComponent("microphone.caf"))
    try file.read(into: pcm, frameCount: 480)
    XCTAssertTrue((0..<480).allSatisfy { abs(pcm.floatChannelData![0][$0] - 0.25) < 0.00001 })
    // Late tap callbacks after Stop cannot reopen or append to the completed recording.
    output.appendMicrophone(pcm, hostTime: output.origin)
    XCTAssertNil(output.error)
  }
}
