import AVFoundation
import XCTest

@testable import CaptureCore

/// What Anarlog's adapter reads: framed events and PCM on the tracks' own frame positions.
final class LiveOutputTests: XCTestCase {
  final class Collector: @unchecked Sendable {
    private let lock = NSLock()
    private var data = Data()
    var delay: TimeInterval = 0

    func write(_ chunk: Data) {
      if delay > 0 { Thread.sleep(forTimeInterval: delay) }
      lock.lock()
      data.append(chunk)
      lock.unlock()
    }

    enum Message {
      case event(EventLine)
      case pcm(track: UInt8, frame: Int64, samples: [Float])
    }

    var messages: [Message] {
      lock.lock()
      let bytes = [UInt8](data)
      lock.unlock()
      var result: [Message] = []
      var at = 0
      while at + 5 <= bytes.count {
        let length = Int(Self.littleEndian(bytes, at + 1))
        let payload = Array(bytes[(at + 5)..<(at + 5 + length)])
        if bytes[at] == WireFrame.event {
          result.append(.event(EventLine.parse(String(decoding: payload, as: UTF8.self))!))
        } else {
          let frame = payload[1..<9].enumerated().reduce(UInt64(0)) {
            $0 | UInt64($1.element) << (8 * UInt64($1.offset))
          }
          let samples = stride(from: 9, to: payload.count, by: 4).map { index in
            Float(bitPattern: Self.littleEndian(payload, index))
          }
          result.append(.pcm(track: payload[0], frame: Int64(frame), samples: samples))
        }
        at += 5 + length
      }
      XCTAssertEqual(at, bytes.count, "a message was cut")
      return result
    }

    // One shift per statement: the inline expression is too slow for Swift to type-check.
    static func littleEndian(_ bytes: [UInt8], _ at: Int) -> UInt32 {
      var word: UInt32 = 0
      for offset in 0..<4 {
        word |= UInt32(bytes[at + offset]) << UInt32(8 * offset)
      }
      return word
    }

    var events: [EventLine] {
      messages.compactMap { if case .event(let line) = $0 { line } else { nil } }
    }

    func pcm(_ track: UInt8) -> [(frame: Int64, samples: [Float])] {
      messages.compactMap {
        if case .pcm(track, let frame, let samples) = $0 { (frame, samples) } else { nil }
      }
    }
  }

  var directory: URL!

  override func setUpWithError() throws {
    directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: false)
  }

  override func tearDownWithError() throws {
    try? FileManager.default.removeItem(at: directory)
  }

  func output(_ collector: Collector, capacity: Int = 256) throws -> AudioOutput {
    try AudioOutput(
      directory: directory, captureID: "capture-1", processID: 0, bundleID: "",
      microphoneID: nil, live: LiveOutput(capacity: capacity, write: collector.write))
  }

  func buffer(_ count: Int, sample: Float) -> AVAudioPCMBuffer {
    let format = AVAudioFormat(standardFormatWithSampleRate: 48_000, channels: 1)!
    let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: AVAudioFrameCount(count))!
    buffer.frameLength = buffer.frameCapacity
    buffer.floatChannelData![0].initialize(repeating: sample, count: count)
    return buffer
  }

  func read(_ name: String) throws -> [Float] {
    let file = try AVAudioFile(forReading: directory.appendingPathComponent(name))
    let pcm = AVAudioPCMBuffer(
      pcmFormat: file.processingFormat, frameCapacity: AVAudioFrameCount(file.length))!
    try file.read(into: pcm)
    return Array(UnsafeBufferPointer(start: pcm.floatChannelData![0], count: Int(pcm.frameLength)))
  }

  func testLiveAudioIsTheTracksOwnSamplesOnTheSharedTimeline() throws {
    let collector = Collector()
    let output = try output(collector)
    output.appendMicrophone(buffer(4800, sample: 0.25), hostTime: output.origin)
    output.appendRemote(buffer(2400, sample: 0.5), hostTime: output.origin + 0.05)
    try output.end()
    let ended = try output.finish(reason: "user_stopped")
    let mic = collector.pcm(0)
    let remote = collector.pcm(1)
    XCTAssertEqual(mic.first?.frame, 0)
    XCTAssertEqual(remote.first?.frame, 2400)
    XCTAssertEqual(mic.map(\.samples.count).reduce(0, +), 4800)
    XCTAssertEqual(remote.map(\.samples.count).reduce(0, +), 2400)
    // The live stream and the CAF hold the same samples at the same positions.
    let micFile = try read("microphone.caf")
    for run in mic {
      for (offset, sample) in run.samples.enumerated() {
        XCTAssertEqual(sample, micFile[Int(run.frame) + offset])
      }
    }
    XCTAssertEqual(collector.events.first?.event, "prepared")
    XCTAssertEqual(collector.events.last, ended)
    XCTAssertEqual(ended.value("microphoneFrames"), "4800")
  }

  func testPausedAudioNeverReachesTheLiveStream() throws {
    let collector = Collector()
    let output = try output(collector)
    try output.pause()
    Thread.sleep(forTimeInterval: 0.2)
    try output.resume()
    // A late callback covering the whole pause.
    output.appendMicrophone(buffer(36_000, sample: 0.5), hostTime: output.origin)
    try output.end()
    _ = try output.finish(reason: "user_stopped")
    let paused = try XCTUnwrap(collector.events.first { $0.event == "paused" })
    let resumed = try XCTUnwrap(collector.events.first { $0.event == "resumed" })
    let from = try XCTUnwrap(paused.value("frame").flatMap(Int64.init))
    let to = try XCTUnwrap(resumed.value("frame").flatMap(Int64.init))
    XCTAssertGreaterThan(to - from, 9_000)
    for run in collector.pcm(0) {
      let end = run.frame + Int64(run.samples.count)
      XCTAssertFalse(
        run.frame < to - 1 && end > from + 1, "PCM \(run.frame)..<\(end) overlaps the pause")
    }
    XCTAssertGreaterThan(collector.pcm(0).count, 0)
  }

  func testTheWatermarkAdvancesThroughAPause() throws {
    let collector = Collector()
    let output = try output(collector)
    try output.started(EventLine("started"))
    try output.pause()
    Thread.sleep(forTimeInterval: 0.9)
    try output.end()
    _ = try output.finish(reason: "user_stopped")
    let ticks = collector.events.filter { $0.event == "tick" }.compactMap {
      $0.value("frame").flatMap(Int64.init)
    }
    XCTAssertGreaterThan(ticks.count, 2)
    XCTAssertEqual(ticks, ticks.sorted())
    XCTAssertEqual(Set(ticks).count, ticks.count)
    // Half a second behind the host clock: no live sample is final before the device delivered it.
    XCTAssertLessThan(ticks.last!, 48_000 * 9 / 10 - 48_000 / 2 + 4_800)
  }

  /// A reader that falls behind fails the capture loudly; the tracks still finish.
  func testAReaderThatFallsBehindFailsTheCapture() throws {
    let collector = Collector()
    collector.delay = 0.05
    let output = try output(collector, capacity: 4)
    for index in 0..<20 {
      output.appendRemote(buffer(480, sample: 0.25), hostTime: output.origin + Double(index) * 0.01)
    }
    try output.end()
    _ = try output.finish(reason: "capture_failed")
    XCTAssertEqual((output.error as? CaptureError)?.code, "live_output_overrun")
    let journal = try String(
      contentsOf: directory.appendingPathComponent("capture.journal"), encoding: .utf8)
    XCTAssertTrue(journal.contains("event=live-output-overrun"))
    XCTAssertTrue(journal.contains("event=ended"))
    // What was written before the failure stays; nothing after it is admitted.
    let remote = try read("remote.caf")
    XCTAssertGreaterThan(remote.count, 0)
    XCTAssertLessThan(remote.count, 9_600)
  }
}

final class WireTests: XCTestCase {
  func testEventLinesRoundTripAnyValue() {
    let line = EventLine("started").with("callUrl", "https://meet.google.com/abc-defg-hij?x=1&y=2")
      .with("name", "Alex Kim = 50% (Host)\nnext").with("pid", Int32(42)).with("ok", true)
      .with("missing", nil as String?).with("peak", 0.25)
    XCTAssertFalse(line.text.contains("\n"))
    XCTAssertEqual(line.text.split(separator: " ").count, 6)
    XCTAssertEqual(EventLine.parse(line.text), line)
    XCTAssertNil(line.value("missing"))
    XCTAssertEqual(line.value("ok"), "true")
    XCTAssertNil(EventLine.parse("pid=1 event=started"))
  }

  func testLongRunsSplitIntoBoundedMessages() {
    let samples = [Float](repeating: 0.5, count: WireFrame.maxSamples + 10)
    let messages = samples.withUnsafeBufferPointer {
      WireFrame.encode(track: 1, frame: 100, samples: $0)
    }
    XCTAssertEqual(messages.count, 2)
    XCTAssertEqual(messages[0].count, 5 + 9 + WireFrame.maxSamples * 4)
    XCTAssertEqual(messages[1].count, 5 + 9 + 10 * 4)
    // The second run starts where the first ended.
    let second = [UInt8](messages[1])
    XCTAssertEqual(second[0], WireFrame.pcm)
    XCTAssertEqual(second[5], 1)
    let frame = second[6..<14].enumerated().reduce(UInt64(0)) {
      $0 | UInt64($1.element) << (8 * UInt64($1.offset))
    }
    XCTAssertEqual(frame, UInt64(100 + WireFrame.maxSamples))
  }
}
