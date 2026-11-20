import AVFoundation
import Foundation

/// The track's file refused a write, as on a full disk. Unlike a bad buffer, it ends the recording.
struct AudioWriteError: LocalizedError {
  let underlying: Error
  var errorDescription: String? {
    "Cannot write the recording: \(underlying.localizedDescription)"
  }
}

/// Serial-queue owned. Float CAF stores each source independently without a duration header limit.
public final class AudioTrack {
  public static let sampleRate = 48_000.0
  /// How far a device's clock may wander from the host clock before the track re-anchors to
  /// host time. A device never runs at exactly its nominal rate; within this, samples are placed
  /// by count, so none are dropped or doubled. Beyond it, as after a dropped IO cycle, the track
  /// keeps silence for the gap or skips the overlap.
  static let clockTolerance = 0.005
  private let format = AVAudioFormat(standardFormatWithSampleRate: sampleRate, channels: 1)!
  private var file: AVAudioFile?
  private var converter: AVAudioConverter?
  /// Where the current run's input ends on its own sample clock, anchored at the run's host time.
  private var inputEnd: Double?
  private var outputTime = 0.0
  private var origin = 0.0
  public private(set) var writtenFrames: Int64 = 0
  public private(set) var capturedFrames: Int64 = 0
  public private(set) var peak: Float = 0

  private let samples: ((AVAudioPCMBuffer, Int64) -> Void)?

  public init(url: URL, samples: ((AVAudioPCMBuffer, Int64) -> Void)? = nil) throws {
    self.samples = samples
    file = try AVAudioFile(forWriting: url, settings: format.settings)
  }

  public func append(_ input: AVAudioPCMBuffer, hostTime: Double, timeline: CaptureTimeline) throws
  {
    guard file != nil else { throw CaptureError.invalid("Audio file is closed") }
    guard input.frameLength > 0 else { return }
    guard input.format.sampleRate > 0, input.frameLength <= 65_536,
      hostTime.isFinite, abs(hostTime - timeline.origin) <= 43_200
    else { throw CaptureError.invalid("Unsupported audio buffer or timestamp") }
    origin = timeline.origin
    let ranges = try timeline.ranges(
      start: hostTime, frames: Int(input.frameLength), rate: input.format.sampleRate)
    // Clip before resampling: paused sound must never enter the converter's filter history.
    for range in ranges {
      let start = hostTime + Double(range.lowerBound) / input.format.sampleRate
      let end = hostTime + Double(range.upperBound) / input.format.sampleRate
      if let inputEnd, end <= inputEnd + 0.000001 { continue }
      if converter?.inputFormat != input.format || inputEnd == nil
        || abs(start - inputEnd!) > Self.clockTolerance
      {
        try flush()
        converter = AVAudioConverter(from: input.format, to: format)
        converter?.primeMethod = .normal
        outputTime = start
      }
      guard let converter else { throw CaptureError.invalid("Cannot convert the audio format") }
      let chunk = try Self.copy(input, range: range)
      inputEnd = (inputEnd ?? start) + Double(range.count) / input.format.sampleRate
      var supplied = false
      try convert(converter) { _, status in
        if supplied {
          status.pointee = .noDataNow
          return nil
        }
        supplied = true
        status.pointee = .haveData
        return chunk
      }
    }
    if ranges.isEmpty { try flush() }
  }

  static func copy(_ input: AVAudioPCMBuffer, range: Range<Int>) throws -> AVAudioPCMBuffer {
    guard
      let result = AVAudioPCMBuffer(
        pcmFormat: input.format, frameCapacity: AVAudioFrameCount(range.count))
    else { throw CaptureError.invalid("Cannot allocate audio buffer") }
    result.frameLength = result.frameCapacity
    let source = UnsafeMutableAudioBufferListPointer(input.mutableAudioBufferList)
    let target = UnsafeMutableAudioBufferListPointer(result.mutableAudioBufferList)
    for index in 0..<source.count {
      let stride = Int(source[index].mDataByteSize) / Int(input.frameLength)
      guard let from = source[index].mData, let to = target[index].mData, stride > 0 else {
        throw CaptureError.invalid("Missing audio samples")
      }
      memcpy(to, from.advanced(by: range.lowerBound * stride), range.count * stride)
    }
    return result
  }

  private func convert(
    _ converter: AVAudioConverter, input: @escaping AVAudioConverterInputBlock
  ) throws {
    // A converter buffers across callbacks. Place its OUTPUT on a continuous clock,
    // rather than incorrectly assigning every output chunk the latest input timestamp.
    for _ in 0..<64 {
      let output = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: 4096)!
      var error: NSError?
      let status = converter.convert(to: output, error: &error, withInputFrom: input)
      if let error { throw error }
      guard status != .error else { throw CaptureError.invalid("Audio conversion failed") }
      try write(output)
      if status == .endOfStream || status == .inputRanDry { return }
    }
    throw CaptureError.invalid("Audio converter exceeded its bounded output budget")
  }

  private func write(_ buffer: AVAudioPCMBuffer) throws {
    guard let inputEnd else { return }
    let count = min(
      Int(buffer.frameLength), max(0, Int(((inputEnd - outputTime) * Self.sampleRate).rounded())))
    let offset = Int64(((outputTime - origin) * Self.sampleRate).rounded())
    outputTime += Double(buffer.frameLength) / Self.sampleRate
    let skip = max(0, writtenFrames - offset)
    guard skip < count else { return }
    try pad(to: max(writtenFrames, offset))
    let length = count - Int(skip)
    let chunk = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: AVAudioFrameCount(length))!
    chunk.frameLength = chunk.frameCapacity
    let source = buffer.floatChannelData![0].advanced(by: Int(skip))
    chunk.floatChannelData![0].update(from: source, count: length)
    for sample in UnsafeBufferPointer(start: source, count: length) {
      guard sample.isFinite else { throw CaptureError.invalid("Non-finite audio sample") }
      peak = max(peak, abs(sample))
    }
    try write(chunk, to: file!)
    samples?(chunk, writtenFrames)
    writtenFrames += Int64(length)
    capturedFrames += Int64(length)
  }

  private func write(_ buffer: AVAudioPCMBuffer, to file: AVAudioFile) throws {
    do { try file.write(from: buffer) } catch { throw AudioWriteError(underlying: error) }
  }

  public func flush() throws {
    if let converter {
      try convert(converter) { _, status in
        status.pointee = .endOfStream
        return nil
      }
    }
    converter = nil
    inputEnd = nil
  }

  /// Drops the conversion in progress after a failed append; the next buffer starts afresh.
  func reset() {
    converter = nil
    inputEnd = nil
  }

  public func pad(to target: Int64) throws {
    guard file != nil else { throw CaptureError.invalid("Audio file is closed") }
    guard target >= writtenFrames, target - writtenFrames <= Int64(Self.sampleRate * 43_200)
    else { throw CaptureError.invalid("Invalid audio timeline gap") }
    while writtenFrames < target {
      let count = AVAudioFrameCount(min(4096, target - writtenFrames))
      let silence = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: count)!
      silence.frameLength = count
      silence.floatChannelData![0].initialize(repeating: 0, count: Int(count))
      try write(silence, to: file!)
      writtenFrames += Int64(count)
    }
  }

  public func close() throws {
    guard file != nil else { return }
    defer {
      file?.close()
      file = nil
    }
    try flush()
  }
}
