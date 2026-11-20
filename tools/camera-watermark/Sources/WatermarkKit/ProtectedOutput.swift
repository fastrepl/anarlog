import Foundation

/// Whether Anarlog Camera's frames, as the extension sent them to meeting apps, hide the room with
/// the Background Anarlog last asked for. Anarlog turns a meeting app's own blur or background off
/// only on this receipt, so the room never shows in between. The agent's desired look isn't enough:
/// a frame composed before the look changed can still be on its way out.
///
/// Each frame the helper enqueues gets a sequence number and the request it was drawn for
/// (`AgentState.backgroundID`). The extension reports a frame's number only once it has handed that
/// frame to the meeting apps' stream (`CameraIdentity.outputReceiptsPropertyName`). Frames leave in
/// order, so once a frame drawn for the current request goes out, every later one is drawn for it
/// too, until the request changes. Whether a meeting app had already buffered or encoded an older
/// frame is beyond what Anarlog can see.
public struct ProtectedOutput: Sendable {
  /// Without a newer frame out within this long, the output has stalled and proves nothing.
  public static let maxAge: TimeInterval = 1
  /// Frames enqueued but not yet reported; older ones are forgotten (dropped somewhere).
  static let maxInFlight = 64

  private struct Frame {
    let sequence: UInt64
    let backgroundID: String
    let hidesRoom: Bool
  }

  /// The number the next enqueued frame gets.
  public private(set) var nextSequence: UInt64
  private var inFlight: [Frame] = []
  private var lastOut: (frame: Frame, at: TimeInterval)?

  /// Numbers start somewhere no other numbering would, so a report that isn't ours never matches.
  public init(firstSequence: UInt64 = UInt64.random(in: 1 << 40..<1 << 52)) {
    nextSequence = firstSequence
  }

  /// The frame numbered `nextSequence` is in the sink's queue, drawn for `backgroundID`.
  public mutating func enqueued(backgroundID: String, hidesRoom: Bool) {
    inFlight.append(Frame(sequence: nextSequence, backgroundID: backgroundID, hidesRoom: hidesRoom))
    if inFlight.count > Self.maxInFlight { inFlight.removeFirst(inFlight.count - Self.maxInFlight) }
    nextSequence += 1
  }

  /// The extension sent frame `sequence` out at `time`. Unknown numbers (an extension too old to
  /// report what it sent, or a frame from before `reset`) are ignored.
  public mutating func sent(_ sequence: UInt64, at time: TimeInterval) {
    guard let index = inFlight.firstIndex(where: { $0.sequence == sequence }) else { return }
    lastOut = (inFlight[index], time)
    inFlight.removeFirst(index + 1)
  }

  /// `kCMIOSampleBufferAttachmentKey_SequenceNumber`, the attachment that carries a frame's
  /// number to the extension. `CMIOSampleBuffer.h` isn't in CoreMediaIO's Swift module, so it is
  /// looked up by name; nil (never seen) leaves frames unnumbered and so never counted as sent.
  public static let sequenceNumberKey: CFString? = dlsym(
    UnsafeMutableRawPointer(bitPattern: -2), "kCMIOSampleBufferAttachmentKey_SequenceNumber"
  ).map { $0.assumingMemoryBound(to: CFString.self).pointee }

  /// The sink stopped: nothing in flight will be reported, and nothing goes out.
  public mutating func reset() {
    inFlight.removeAll()
    lastOut = nil
  }

  /// True while the latest frame out, within `maxAge` of `now`, hid the room for `backgroundID`.
  public func hidesRoom(for backgroundID: String, now: TimeInterval) -> Bool {
    guard !backgroundID.isEmpty, let lastOut else { return false }
    return lastOut.frame.backgroundID == backgroundID && lastOut.frame.hidesRoom
      && now - lastOut.at <= Self.maxAge
  }
}
