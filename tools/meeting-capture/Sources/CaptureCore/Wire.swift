import Foundation

/// One `event=<name> key=value ...` line. Values are percent-encoded so a line never holds a space,
/// `=` or newline of its own. Anarlog's adapter parses it without a JSON dependency.
public struct EventLine: Equatable {
  public private(set) var fields: [(key: String, value: String)]

  public init(_ event: String) {
    fields = [("event", event)]
  }

  public var event: String { fields[0].value }

  public func with(_ key: String, _ value: String?) -> EventLine {
    guard let value else { return self }
    var line = self
    line.fields.append((key, value))
    return line
  }

  public func with(_ key: String, _ value: Bool) -> EventLine {
    with(key, value ? "true" : "false")
  }
  public func with(_ key: String, _ value: Int) -> EventLine { with(key, String(value)) }
  public func with(_ key: String, _ value: Int32) -> EventLine { with(key, String(value)) }
  public func with(_ key: String, _ value: Int64) -> EventLine { with(key, String(value)) }
  public func with(_ key: String, _ value: Double) -> EventLine {
    with(key, value.isFinite ? String(value) : nil)
  }
  public func with(_ key: String, _ value: Float) -> EventLine { with(key, Double(value)) }

  public func value(_ key: String) -> String? { fields.first { $0.key == key }?.value }

  public var text: String {
    fields.map { "\($0.key)=\(Self.encode($0.value))" }.joined(separator: " ")
  }

  private static let allowed = CharacterSet(
    charactersIn: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~:/@+")

  static func encode(_ value: String) -> String {
    value.addingPercentEncoding(withAllowedCharacters: allowed) ?? ""
  }

  /// The inverse of `text`, for tests and journal readers.
  public static func parse(_ text: String) -> EventLine? {
    var pairs: [(String, String)] = []
    for token in text.split(separator: " ", omittingEmptySubsequences: true) {
      guard let equals = token.firstIndex(of: "="),
        let value = String(token[token.index(after: equals)...]).removingPercentEncoding
      else { return nil }
      pairs.append((String(token[..<equals]), value))
    }
    guard let first = pairs.first, first.0 == "event" else { return nil }
    var line = EventLine(first.1)
    line.fields = pairs.map { (key: $0.0, value: $0.1) }
    return line
  }

  public static func == (lhs: EventLine, rhs: EventLine) -> Bool {
    lhs.fields.elementsEqual(rhs.fields) { $0.key == $1.key && $0.value == $1.value }
  }
}

/// The stdout framing: `kind: u8`, `length: u32 LE`, `payload`. `E` carries one event line,
/// `P` one run of 48 kHz mono PCM: `track: u8`, `frame: u64 LE`, then f32 LE samples.
public enum WireFrame {
  public static let event: UInt8 = 0x45
  public static let pcm: UInt8 = 0x50
  /// Keeps each message well under the adapter's 1 MiB payload limit.
  public static let maxSamples = 65_536

  public static func encode(_ line: EventLine) -> Data {
    frame(event, Data(line.text.utf8))
  }

  public static func encode(track: UInt8, frame first: Int64, samples: UnsafeBufferPointer<Float>)
    -> [Data]
  {
    stride(from: 0, to: samples.count, by: maxSamples).map { offset in
      let count = min(maxSamples, samples.count - offset)
      var payload = Data(capacity: 9 + count * 4)
      payload.append(track)
      withUnsafeBytes(of: UInt64(first + Int64(offset)).littleEndian) {
        payload.append(contentsOf: $0)
      }
      for sample in samples[offset..<(offset + count)] {
        withUnsafeBytes(of: sample.bitPattern.littleEndian) { payload.append(contentsOf: $0) }
      }
      return frame(pcm, payload)
    }
  }

  private static func frame(_ kind: UInt8, _ payload: Data) -> Data {
    var data = Data(capacity: 5 + payload.count)
    data.append(kind)
    withUnsafeBytes(of: UInt32(payload.count).littleEndian) { data.append(contentsOf: $0) }
    data.append(payload)
    return data
  }
}

/// Live output to Anarlog, off the audio writer's queue so a slow reader never stalls the tracks.
/// Bounded: when Anarlog falls `capacity` messages behind, `send` refuses and the capture fails
/// loudly rather than dropping live audio in silence. The CAF tracks keep everything up to then.
public final class LiveOutput: @unchecked Sendable {
  private let queue = DispatchQueue(label: "so.anarlog.meeting-capture.live")
  private let slots: DispatchSemaphore
  private let write: (Data) -> Void

  public init(capacity: Int = 256, write: @escaping (Data) -> Void = LiveOutput.standardOutput) {
    slots = DispatchSemaphore(value: capacity)
    self.write = write
  }

  /// A closed pipe means Anarlog is gone; the helper notices through stdin EOF and stops.
  public static func standardOutput(_ data: Data) {
    try? FileHandle.standardOutput.write(contentsOf: data)
  }

  @discardableResult
  public func send(_ data: Data, wait: DispatchTime = .now()) -> Bool {
    guard slots.wait(timeout: wait) == .success else { return false }
    queue.async { [self] in
      defer { slots.signal() }
      write(data)
    }
    return true
  }

  @discardableResult
  public func send(_ line: EventLine, wait: DispatchTime = .now()) -> Bool {
    send(WireFrame.encode(line), wait: wait)
  }

  /// Every message sent before this call has been written.
  public func flush() { queue.sync {} }
}
