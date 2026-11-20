import Foundation

public enum CaptureError: Error, LocalizedError {
  case invalid(String)
  /// A refusal Anarlog acts on by `code`, such as `microphone_not_default`.
  case refused(code: String, message: String)

  public var errorDescription: String? {
    switch self {
    case .invalid(let message), .refused(_, let message): return message
    }
  }

  public var code: String {
    switch self {
    case .invalid: return "capture_failed"
    case .refused(let code, _): return code
    }
  }
}

/// All sources use the Core Media host clock, never wall time or callback arrival time.
/// Pauses remain silence in the files so metadata timestamps never need rebasing.
public struct CaptureTimeline {
  public let origin: Double
  private var windows: [(start: Double, end: Double?)]
  public private(set) var ended = false
  public private(set) var stoppedAt: Double?

  public init(origin: Double) throws {
    guard origin.isFinite, origin >= 0 else { throw CaptureError.invalid("Invalid host clock") }
    self.origin = origin
    windows = [(origin, nil)]
  }

  public var paused: Bool { windows.last?.end != nil }

  /// Reject a slow UI read that began before the most recent resume boundary.
  public func acceptsObservation(startedAt time: Double) -> Bool {
    !ended && !paused && time.isFinite && time >= windows.last!.start
  }

  public mutating func pause(at time: Double) throws {
    try validate(time)
    guard !ended, !paused else { throw CaptureError.invalid("Capture is not recording") }
    windows[windows.count - 1].end = time
  }

  public mutating func resume(at time: Double) throws {
    try validate(time)
    guard !ended, paused else { throw CaptureError.invalid("Capture is not paused") }
    windows.append((time, nil))
  }

  public mutating func stop(at time: Double) throws {
    try validate(time)
    guard !ended else { return }
    if !paused { windows[windows.count - 1].end = time }
    stoppedAt = time
    ended = true
  }

  private func validate(_ time: Double) throws {
    let last = windows.last!
    guard time.isFinite, time >= (last.end ?? last.start) else {
      throw CaptureError.invalid("Capture clock moved backwards")
    }
  }

  public func ranges(start: Double, frames: Int, rate: Double) throws -> [Range<Int>] {
    guard start.isFinite, frames >= 0, rate.isFinite, rate > 0 else {
      throw CaptureError.invalid("Invalid audio timestamp or format")
    }
    let end = start + Double(frames) / rate
    return windows.compactMap { window in
      let low = max(start, window.start)
      let high = min(end, window.end ?? end)
      guard high > low else { return nil }
      // Include only samples whose entire interval is inside a recording window.
      let first = max(0, min(frames, Int(ceil((low - start) * rate - 0.00001))))
      let last = max(0, min(frames, Int(floor((high - start) * rate + 0.00001))))
      return first < last ? first..<last : nil
    }
  }
}
