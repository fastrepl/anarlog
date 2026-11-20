import CoreMediaIO
import CoreVideo
import Foundation
import IOKit.audio
import WatermarkKit
import os.log

private let log = Logger(subsystem: CameraIdentity.extensionBundleID, category: "provider")
private let consumersProperty = CMIOExtensionProperty(
  rawValue: CameraIdentity.consumersPropertyName)
private let outputReceiptsProperty = CMIOExtensionProperty(
  rawValue: CameraIdentity.outputReceiptsPropertyName)

final class ProviderSource: NSObject, CMIOExtensionProviderSource {
  private(set) var provider: CMIOExtensionProvider!
  private var device: DeviceSource!

  override init() {
    super.init()
    provider = CMIOExtensionProvider(source: self, clientQueue: nil)
    device = DeviceSource()
    do {
      try provider.addDevice(device.device)
    } catch {
      fatalError("Couldn't add the Anarlog Camera device: \(error)")
    }
  }

  func connect(to client: CMIOExtensionClient) throws {}
  func disconnect(from client: CMIOExtensionClient) {
    // A writer that crashed never stops its stream; free the sink for the next helper.
    device.clientDisconnected(client.clientID)
  }

  var availableProperties: Set<CMIOExtensionProperty> { [.providerManufacturer] }

  func providerProperties(forProperties properties: Set<CMIOExtensionProperty>) throws
    -> CMIOExtensionProviderProperties
  {
    let state = CMIOExtensionProviderProperties(dictionary: [:])
    if properties.contains(.providerManufacturer) {
      state.manufacturer = CameraIdentity.manufacturer
    }
    return state
  }

  func setProviderProperties(_ providerProperties: CMIOExtensionProviderProperties) throws {}
}

/// One camera, "Anarlog Camera", with a source stream meeting apps read and a sink stream the
/// helper writes. The helper's frames go out the moment they arrive; a 30 fps timer sends the
/// idle frame while none has arrived for `liveFrameMaxAge`.
final class DeviceSource: NSObject, CMIOExtensionDeviceSource {
  private(set) var device: CMIOExtensionDevice!
  private var source: StreamSource!
  private var sink: StreamSink!

  private let queue = DispatchQueue(label: "so.anarlog.camera.extension.frames")
  private var timer: DispatchSourceTimer?
  private let format: CMFormatDescription
  private let idleFrame: CVPixelBuffer
  /// When the helper's last frame arrived; on `queue`. The idle frame shows once it is stale.
  private var lastLiveFrameAt: TimeInterval?
  /// Guarded by `lock`: CMIO reads it from its own queue, sometimes inside our notification.
  private var consumers: [UUID: String] = [:]
  /// Who may write to the sink: the helper signed by this extension's own team. Guarded by `lock`.
  private var gate = SinkGate(teamID: SignedClient.teamID(pid: getpid()))
  private let lock = NSLock()

  override init() {
    var description: CMFormatDescription?
    CMVideoFormatDescriptionCreate(
      allocator: kCFAllocatorDefault, codecType: kCVPixelFormatType_32BGRA,
      width: Int32(CameraIdentity.width), height: Int32(CameraIdentity.height), extensions: nil,
      formatDescriptionOut: &description)
    format = description!
    idleFrame = Watermark.makeBuffer()!
    Watermark.withContext(on: idleFrame) { ctx, size in Watermark.drawIdle(in: ctx, frame: size) }
    super.init()
    if gate.teamID == nil {
      log.error("not signed by a team: refusing every sink writer, so only the idle frame shows")
    }

    device = CMIOExtensionDevice(
      localizedName: CameraIdentity.deviceName, deviceID: CameraIdentity.deviceUUID,
      legacyDeviceID: nil, source: self)
    let duration = CMTime(value: 1, timescale: CameraIdentity.framesPerSecond)
    let streamFormat = CMIOExtensionStreamFormat(
      formatDescription: format, maxFrameDuration: duration, minFrameDuration: duration,
      validFrameDurations: nil)
    source = StreamSource(device: self, format: streamFormat)
    sink = StreamSink(device: self, format: streamFormat)
    do {
      try device.addStream(source.stream)
      try device.addStream(sink.stream)
    } catch {
      fatalError("Couldn't add the Anarlog Camera streams: \(error)")
    }
  }

  var availableProperties: Set<CMIOExtensionProperty> {
    [.deviceTransportType, .deviceModel, consumersProperty, outputReceiptsProperty]
  }

  func deviceProperties(forProperties properties: Set<CMIOExtensionProperty>) throws
    -> CMIOExtensionDeviceProperties
  {
    let state = CMIOExtensionDeviceProperties(dictionary: [:])
    if properties.contains(.deviceTransportType) {
      state.transportType = kIOAudioDeviceTransportTypeVirtual
    }
    if properties.contains(.deviceModel) { state.model = CameraIdentity.deviceName }
    if properties.contains(consumersProperty) {
      state.setPropertyState(
        CMIOExtensionPropertyState(value: consumersValue() as AnyObject),
        forProperty: consumersProperty)
    }
    if properties.contains(outputReceiptsProperty) {
      state.setPropertyState(
        CMIOExtensionPropertyState(value: CameraIdentity.outputReceiptsValue as AnyObject),
        forProperty: outputReceiptsProperty)
    }
    return state
  }

  func setDeviceProperties(_ deviceProperties: CMIOExtensionDeviceProperties) throws {}

  // MARK: Source: meeting apps

  /// Records each app as it is allowed to stream, not only the one that started the stream, so
  /// a second app (Zoom after Settings' preview) shows too; `clientDisconnected` drops it.
  func sourceAuthorized(_ client: CMIOExtensionClient) {
    let id = Self.identity(of: client)
    let changed = lock.withLock { consumers.updateValue(id, forKey: client.clientID) != id }
    if changed { publishConsumers() }
  }

  func sourceStarted() {
    queue.async { self.startTimerIfNeeded() }
  }

  /// The client's signing identifier as the kernel recorded it; CMIO's own `signingID` reads
  /// "unknown" for Developer ID apps.
  private static func identity(of client: CMIOExtensionClient) -> String {
    if let id = SignedClient.signingID(pid: client.pid) { return id }
    if let id = client.signingID, id != "unknown" { return id }
    return "pid \(client.pid)"
  }

  func sourceStopped() {
    // `stopStream` doesn't say who stopped; the source stream runs while anyone streams.
    lock.withLock { consumers.removeAll() }
    publishConsumers()
    queue.async {
      self.timer?.cancel()
      self.timer = nil
    }
  }

  private func consumersValue() -> String {
    lock.withLock { ConsumerList.encode(consumers.values.sorted()) }
  }

  private func publishConsumers() {
    let value = consumersValue()
    log.info("consumers changed: \(value, privacy: .public)")
    device.notifyPropertiesChanged([
      consumersProperty: CMIOExtensionPropertyState(value: value as AnyObject)
    ])
  }

  private func startTimerIfNeeded() {
    guard timer == nil else { return }
    let timer = DispatchSource.makeTimerSource(queue: queue)
    timer.schedule(deadline: .now(), repeating: 1.0 / Double(CameraIdentity.framesPerSecond))
    timer.setEventHandler { [weak self] in self?.sendIdleFrameIfStale() }
    timer.resume()
    self.timer = timer
  }

  private func sendIdleFrameIfStale() {
    let now = ProcessInfo.processInfo.systemUptime
    if let last = lastLiveFrameAt, now - last < CameraIdentity.liveFrameMaxAge { return }
    send(idleFrame)
  }

  /// One frame out to every meeting app, stamped with the time it leaves; that time, or nil when
  /// it couldn't go.
  @discardableResult
  private func send(_ buffer: CVPixelBuffer) -> UInt64? {
    let hostTime = DispatchTime.now().uptimeNanoseconds
    var timing = CMSampleTimingInfo(
      duration: CMTime(value: 1, timescale: CameraIdentity.framesPerSecond),
      presentationTimeStamp: CMTime(value: CMTimeValue(hostTime), timescale: 1_000_000_000),
      decodeTimeStamp: .invalid)
    var formatForBuffer: CMFormatDescription?
    CMVideoFormatDescriptionCreateForImageBuffer(
      allocator: kCFAllocatorDefault, imageBuffer: buffer, formatDescriptionOut: &formatForBuffer)
    guard let formatForBuffer else { return nil }
    var sample: CMSampleBuffer?
    let status = CMSampleBufferCreateReadyWithImageBuffer(
      allocator: kCFAllocatorDefault, imageBuffer: buffer, formatDescription: formatForBuffer,
      sampleTiming: &timing, sampleBufferOut: &sample)
    guard status == noErr, let sample else { return nil }
    source.stream.send(sample, discontinuity: [], hostTimeInNanoseconds: hostTime)
    return hostTime
  }

  // MARK: Sink: the helper

  /// Only Anarlog Camera.app, signed by this extension's team, writes frames, one process at a
  /// time.
  func admitWriter(_ client: CMIOExtensionClient) -> Bool {
    let signingID = SignedClient.signingID(pid: client.pid)
    let decision = lock.withLock {
      gate.admit(
        client: client.clientID, signingID: signingID,
        clientTeamID: SignedClient.verifiedTeam(pid: client.pid, expected: gate.teamID))
    }
    if decision != .accept {
      log.error(
        "refused sink writer \(signingID ?? "pid \(client.pid)", privacy: .public): \(String(describing: decision), privacy: .public)"
      )
    }
    return decision == .accept
  }

  func writerStopped() {
    lock.withLock { gate.release(nil) }
  }

  func clientDisconnected(_ clientID: UUID) {
    let removed = lock.withLock {
      gate.release(clientID)
      return consumers.removeValue(forKey: clientID) != nil
    }
    if removed { publishConsumers() }
  }

  /// A frame from the helper. `sent` gets the time it went out to the meeting apps, and only
  /// then: never for a frame nobody streams or one that couldn't be sent, so the helper's receipt
  /// (`ProtectedOutput`) means the frame left, not that it arrived.
  func received(_ sample: CMSampleBuffer, sent: @escaping (UInt64) -> Void) {
    guard let buffer = CMSampleBufferGetImageBuffer(sample) else { return }
    queue.async {
      self.lastLiveFrameAt = ProcessInfo.processInfo.systemUptime
      // Straight out: holding it for the next timer tick would add up to a frame (33 ms).
      guard self.timer != nil, let hostTime = self.send(buffer) else { return }
      sent(hostTime)
    }
  }
}

final class StreamSource: NSObject, CMIOExtensionStreamSource {
  private(set) var stream: CMIOExtensionStream!
  private unowned let device: DeviceSource
  private let format: CMIOExtensionStreamFormat

  init(device: DeviceSource, format: CMIOExtensionStreamFormat) {
    self.device = device
    self.format = format
    super.init()
    stream = CMIOExtensionStream(
      localizedName: "\(CameraIdentity.deviceName) Video",
      streamID: CameraIdentity.sourceStreamUUID,
      direction: .source, clockType: .hostTime, source: self)
  }

  var formats: [CMIOExtensionStreamFormat] { [format] }
  var activeFormatIndex: Int = 0

  var availableProperties: Set<CMIOExtensionProperty> {
    [.streamActiveFormatIndex, .streamFrameDuration]
  }

  func streamProperties(forProperties properties: Set<CMIOExtensionProperty>) throws
    -> CMIOExtensionStreamProperties
  {
    let state = CMIOExtensionStreamProperties(dictionary: [:])
    if properties.contains(.streamActiveFormatIndex) { state.activeFormatIndex = 0 }
    if properties.contains(.streamFrameDuration) {
      state.frameDuration = CMTime(value: 1, timescale: CameraIdentity.framesPerSecond)
    }
    return state
  }

  func setStreamProperties(_ streamProperties: CMIOExtensionStreamProperties) throws {}

  func authorizedToStartStream(for client: CMIOExtensionClient) -> Bool {
    device.sourceAuthorized(client)
    return true
  }

  func startStream() throws {
    device.sourceStarted()
  }

  func stopStream() throws {
    device.sourceStopped()
  }
}

final class StreamSink: NSObject, CMIOExtensionStreamSource {
  private(set) var stream: CMIOExtensionStream!
  private unowned let device: DeviceSource
  private let format: CMIOExtensionStreamFormat
  private var client: CMIOExtensionClient?
  private var running = false

  init(device: DeviceSource, format: CMIOExtensionStreamFormat) {
    self.device = device
    self.format = format
    super.init()
    stream = CMIOExtensionStream(
      localizedName: "\(CameraIdentity.deviceName) Sink", streamID: CameraIdentity.sinkStreamUUID,
      direction: .sink, clockType: .hostTime, source: self)
  }

  var formats: [CMIOExtensionStreamFormat] { [format] }
  var activeFormatIndex: Int = 0

  var availableProperties: Set<CMIOExtensionProperty> {
    [
      .streamActiveFormatIndex, .streamFrameDuration, .streamSinkBufferQueueSize,
      .streamSinkBuffersRequiredForStartup, .streamSinkBufferUnderrunCount, .streamSinkEndOfData,
    ]
  }

  func streamProperties(forProperties properties: Set<CMIOExtensionProperty>) throws
    -> CMIOExtensionStreamProperties
  {
    let state = CMIOExtensionStreamProperties(dictionary: [:])
    if properties.contains(.streamActiveFormatIndex) { state.activeFormatIndex = 0 }
    if properties.contains(.streamFrameDuration) {
      state.frameDuration = CMTime(value: 1, timescale: CameraIdentity.framesPerSecond)
    }
    if properties.contains(.streamSinkBufferQueueSize) { state.sinkBufferQueueSize = 1 }
    if properties.contains(.streamSinkBuffersRequiredForStartup) {
      state.sinkBuffersRequiredForStartup = 1
    }
    return state
  }

  func setStreamProperties(_ streamProperties: CMIOExtensionStreamProperties) throws {}

  func authorizedToStartStream(for client: CMIOExtensionClient) -> Bool {
    guard device.admitWriter(client) else { return false }
    self.client = client
    return true
  }

  func startStream() throws {
    running = true
    consume()
  }

  func stopStream() throws {
    running = false
    client = nil
    device.writerStopped()
  }

  private func consume() {
    guard running, let client else { return }
    stream.consumeSampleBuffer(from: client) { [weak self] sample, sequence, _, _, error in
      guard let self else { return }
      if let sample {
        // The helper's own number, which it matches to the request the frame was drawn for.
        let number = Self.sequenceNumber(of: sample) ?? sequence
        self.device.received(sample) { [weak self] hostTime in
          self?.stream.notifyScheduledOutputChanged(
            CMIOExtensionScheduledOutput(sequenceNumber: number, hostTimeInNanoseconds: hostTime))
        }
      } else if let error {
        log.debug("sink consume: \(error.localizedDescription, privacy: .public)")
      }
      // Ask for the next frame; a short wait keeps an empty queue from spinning.
      DispatchQueue.global().asyncAfter(deadline: .now() + (sample == nil ? 0.01 : 0)) {
        self.consume()
      }
    }
  }

  private static func sequenceNumber(of sample: CMSampleBuffer) -> UInt64? {
    guard let key = ProtectedOutput.sequenceNumberKey else { return nil }
    return (CMGetAttachment(sample, key: key, attachmentModeOut: nil) as? NSNumber)?.uint64Value
  }
}
