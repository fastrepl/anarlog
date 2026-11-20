import AVFoundation
import CaptureCore
import XCTest

final class CaptureTests: XCTestCase {
  func testStopWhilePausedPreservesTerminalTimeWithoutAdmittingPausedAudio() throws {
    var timeline = try CaptureTimeline(origin: 10)
    try timeline.pause(at: 11)
    try timeline.stop(at: 12)
    XCTAssertEqual(timeline.stoppedAt, 12)
    XCTAssertEqual(try timeline.ranges(start: 10, frames: 300, rate: 100), [0..<100])
    try timeline.stop(at: 13)
    XCTAssertEqual(timeline.stoppedAt, 12)
  }

  func testPauseClipsBuffersAcrossBothBoundariesIncludingDelayedCallbacks() throws {
    var timeline = try CaptureTimeline(origin: 100)
    try timeline.pause(at: 101)
    try timeline.resume(at: 102)
    try timeline.stop(at: 103)
    XCTAssertEqual(try timeline.ranges(start: 99.5, frames: 400, rate: 100), [50..<150, 250..<350])
    XCTAssertEqual(try timeline.ranges(start: 101.2, frames: 20, rate: 100), [])
    XCTAssertThrowsError(try timeline.resume(at: 104))
    XCTAssertThrowsError(try timeline.pause(at: .nan))
  }

  func testRejectsInvalidStateTransitionsWithoutChangingCaptureWindows() throws {
    var timeline = try CaptureTimeline(origin: 10)
    XCTAssertThrowsError(try timeline.resume(at: 11))
    try timeline.pause(at: 12)
    XCTAssertThrowsError(try timeline.pause(at: 13))
    XCTAssertThrowsError(try timeline.resume(at: 11))
    try timeline.resume(at: 14)
    XCTAssertEqual(try timeline.ranges(start: 10, frames: 5, rate: 1), [0..<2, 4..<5])
    XCTAssertThrowsError(try timeline.ranges(start: .infinity, frames: 1, rate: 1))
  }

  func testSeparateFilesKeepSharedClockAndDoNotWritePausedOrDuplicateAudio() throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: false)
    defer { try? FileManager.default.removeItem(at: directory) }
    let micURL = directory.appendingPathComponent("mic.caf")
    let remoteURL = directory.appendingPathComponent("remote.caf")
    let mic = try AudioTrack(url: micURL)
    let remote = try AudioTrack(url: remoteURL)
    var timeline = try CaptureTimeline(origin: 10)
    try timeline.pause(at: 10.02)
    try timeline.resume(at: 10.04)
    try timeline.stop(at: 10.06)
    try mic.append(buffer(2880, sample: 0.25), hostTime: 10, timeline: timeline)
    try remote.append(buffer(2400, sample: 0.5), hostTime: 10.01, timeline: timeline)
    // An older callback cannot append duplicate speech or overwrite newer frames.
    try mic.append(buffer(480, sample: 0.9), hostTime: 10, timeline: timeline)
    try mic.close()
    try remote.close()
    let local = try read(micURL)
    let other = try read(remoteURL)
    XCTAssertEqual(local.count, 2880)
    XCTAssertEqual(other.count, 2880)
    XCTAssertTrue(local[0..<960].allSatisfy { abs($0 - 0.25) < 0.0001 })
    XCTAssertTrue(local[960..<1920].allSatisfy { $0 == 0 })
    XCTAssertTrue(local[1920..<2880].allSatisfy { abs($0 - 0.25) < 0.0001 })
    XCTAssertTrue(other[0..<480].allSatisfy { $0 == 0 })
    XCTAssertTrue(other[480..<960].allSatisfy { abs($0 - 0.5) < 0.0001 })
    XCTAssertTrue(other[960..<1920].allSatisfy { $0 == 0 })
    XCTAssertTrue(other[1920..<2880].allSatisfy { abs($0 - 0.5) < 0.0001 })
    XCTAssertEqual(mic.capturedFrames, 1920)
    XCTAssertEqual(remote.capturedFrames, 1440)
    XCTAssertThrowsError(try mic.append(buffer(480), hostTime: 10.1, timeline: timeline))
  }

  func testConvertsDifferentMicrophoneSampleRateWithoutChangingRecordingDuration() throws {
    let url = FileManager.default.temporaryDirectory.appendingPathComponent(
      UUID().uuidString + ".caf")
    defer { try? FileManager.default.removeItem(at: url) }
    let track = try AudioTrack(url: url)
    let timeline = try CaptureTimeline(origin: 20)
    for index in 0..<10 {
      try track.append(
        buffer(4410, sample: 0.2, rate: 44_100), hostTime: 20 + Double(index) * 0.1,
        timeline: timeline)
    }
    try track.close()
    let samples = try read(url)
    XCTAssertEqual(samples.count, 48_000)
    XCTAssertEqual(track.capturedFrames, 48_000)
    XCTAssertEqual(samples[24_000], 0.2, accuracy: 0.01)
  }

  func testPausedSamplesNeverEnterResamplerHistory() throws {
    let url = FileManager.default.temporaryDirectory.appendingPathComponent(
      UUID().uuidString + ".caf")
    defer { try? FileManager.default.removeItem(at: url) }
    let track = try AudioTrack(url: url)
    var timeline = try CaptureTimeline(origin: 10)
    try timeline.pause(at: 10 + 1.0 / 3)
    try timeline.resume(at: 10 + 2.0 / 3)
    try timeline.stop(at: 11)
    let audio = buffer(44_100, sample: 0, rate: 44_100)
    audio.floatChannelData![0].advanced(by: 14_700).initialize(repeating: 1, count: 14_700)
    try track.append(audio, hostTime: 10, timeline: timeline)
    try track.close()
    let samples = try read(url)
    XCTAssertEqual(samples.count, 48_000)
    XCTAssertTrue(samples.allSatisfy { abs($0) < 0.000001 })
  }

  /// A device clock never runs at exactly its nominal rate. Over a minute at 200 ppm off the host
  /// clock, every sample still lands on the track, near where the host clock says it was heard.
  func testADeviceClockOffTheHostClockKeepsEverySampleAndStaysAligned() throws {
    for (ppm, rate) in [(200.0, 48_000.0), (-200.0, 48_000.0), (200.0, 44_100.0)] {
      let url = FileManager.default.temporaryDirectory.appendingPathComponent(
        UUID().uuidString + ".caf")
      defer { try? FileManager.default.removeItem(at: url) }
      let track = try AudioTrack(url: url)
      let timeline = try CaptureTimeline(origin: 50)
      // A fast device delivers 0.1 s of frames in less host time than 0.1 s, a slow one in more.
      let period = 0.1 / (1 + ppm / 1_000_000)
      for index in 0..<600 {
        try track.append(
          buffer(Int(rate / 10), sample: index == 599 ? 0.75 : 0.25, rate: rate),
          hostTime: 50 + Double(index) * period, timeline: timeline)
      }
      try track.close()
      let samples = try read(url)
      let silent = samples.filter { $0 == 0 }.count
      XCTAssertLessThanOrEqual(silent, 2_400, "\(ppm) ppm: \(silent) samples lost to silence")
      let marker = try XCTUnwrap(samples.firstIndex { abs($0 - 0.75) < 0.0001 })
      let heard = Int((599 * period * 48_000).rounded())
      XCTAssertLessThanOrEqual(abs(marker - heard), 480, "\(ppm) ppm: \(marker - heard) frames off")
    }
  }

  /// A headset switching to its 16 kHz headset profile and back, with the silence the switch
  /// costs, keeps each part where the host clock heard it and the track at full length.
  func testARouteSwitchingSampleRateMidRecordingStaysOnTheTimeline() throws {
    let url = FileManager.default.temporaryDirectory.appendingPathComponent(
      UUID().uuidString + ".caf")
    defer { try? FileManager.default.removeItem(at: url) }
    let track = try AudioTrack(url: url)
    var timeline = try CaptureTimeline(origin: 30)
    try timeline.stop(at: 33.6)
    let parts: [(start: Double, rate: Double, sample: Float)] = [
      (30, 48_000, 0.2), (31.3, 16_000, 0.4), (32.6, 48_000, 0.6),
    ]
    for part in parts {
      for index in 0..<10 {
        try track.append(
          buffer(Int(part.rate / 10), sample: part.sample, rate: part.rate),
          hostTime: part.start + Double(index) * 0.1, timeline: timeline)
      }
    }
    try track.close()
    let samples = try read(url)
    XCTAssertEqual(samples.count, 172_800)
    for part in parts {
      let at = Int(((part.start - 30) * 48_000).rounded())
      let heard = samples[at..<(at + 48_000)].indices.filter {
        abs(samples[$0] - part.sample) < 0.01
      }
      // The resampler's filter ramps in and out over a few frames; its latency must not delay
      // the part, which would misalign it with the other track.
      XCTAssertLessThanOrEqual(
        heard.first.map { $0 - at } ?? .max, 24, "part at \(part.start) starts late")
      XCTAssertGreaterThanOrEqual(heard.count, 47_952, "part at \(part.start) lost samples")
    }
    XCTAssertTrue(samples[48_000..<62_400].allSatisfy { $0 == 0 }, "the first switch's gap")
    XCTAssertTrue(samples[110_400..<124_800].allSatisfy { $0 == 0 }, "the second switch's gap")
    XCTAssertTrue(samples[172_800...].isEmpty)
  }

  private func buffer(_ count: Int, sample: Float = 0.1, rate: Double = 48_000) -> AVAudioPCMBuffer
  {
    let format = AVAudioFormat(standardFormatWithSampleRate: rate, channels: 1)!
    let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: AVAudioFrameCount(count))!
    buffer.frameLength = buffer.frameCapacity
    buffer.floatChannelData![0].initialize(repeating: sample, count: count)
    return buffer
  }

  private func read(_ url: URL) throws -> [Float] {
    let file = try AVAudioFile(forReading: url)
    let buffer = AVAudioPCMBuffer(
      pcmFormat: file.processingFormat, frameCapacity: AVAudioFrameCount(file.length))!
    var samples: [Float] = []
    while file.framePosition < file.length {
      try file.read(into: buffer)
      guard buffer.frameLength > 0 else { throw CaptureError.invalid("Unexpected audio EOF") }
      samples.append(
        contentsOf: UnsafeBufferPointer(
          start: buffer.floatChannelData![0], count: Int(buffer.frameLength)))
    }
    return samples
  }
}
