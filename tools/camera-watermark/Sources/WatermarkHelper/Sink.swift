import CoreMediaIO
import Foundation
import WatermarkKit

/// The helper's side of the Anarlog Camera device, through CoreMediaIO's C API: finds the
/// device the extension publishes, reads who streams from it, and writes frames to its sink.
final class SinkClient {
  private(set) var deviceID: CMIOObjectID?
  /// `streamID` and `queue` change on main and are read by `send` on the capture queue; `output`
  /// changes there and on CMIO's thread as the extension reports frames sent.
  private var streamID: CMIOStreamID?
  private var queue: CMSimpleQueue?
  private var output = ProtectedOutput()
  private let lock = NSLock()

  var isStarted: Bool { lock.withLock { queue != nil } }

  /// Looks for the camera again; it appears once the person allows the extension.
  @discardableResult
  func refreshDevice() -> Bool {
    if let deviceID, Self.string(deviceID, kCMIODevicePropertyDeviceUID) != nil { return true }
    stop()
    deviceID = Self.devices().first { id in
      Self.string(id, kCMIODevicePropertyDeviceUID) == CameraIdentity.deviceUUID.uuidString
    }
    return deviceID != nil
  }

  /// The signing IDs of the apps streaming from Anarlog Camera.
  func consumers() -> [String] {
    guard let deviceID,
      let value = Self.string(
        deviceID, CMIOObjectPropertySelector(CameraIdentity.consumersSelector))
    else { return [] }
    return ConsumerList.decode(value)
  }

  func start() -> Bool {
    guard !isStarted, let deviceID, let streamID = Self.sinkStream(of: deviceID) else {
      return isStarted
    }
    var queueRef: Unmanaged<CMSimpleQueue>?
    let status = CMIOStreamCopyBufferQueue(streamID, { _, _, _ in }, nil, &queueRef)
    guard status == noErr, let queueRef else { return false }
    // Only an extension that reports frames as they leave can vouch for what meeting apps get.
    if Self.string(deviceID, CameraIdentity.outputReceiptsSelector)
      == CameraIdentity.outputReceiptsValue
    {
      listenForSentFrames(streamID)
    }
    guard CMIODeviceStartStream(deviceID, streamID) == noErr else { return false }
    lock.withLock {
      self.streamID = streamID
      queue = queueRef.takeUnretainedValue()
    }
    return true
  }

  func stop() {
    let streamID = lock.withLock {
      defer {
        self.streamID = nil
        queue = nil
        output.reset()
      }
      return self.streamID
    }
    if let deviceID, let streamID { CMIODeviceStopStream(deviceID, streamID) }
  }

  /// Queues `sample`, drawn for the Background request `backgroundID`; false when it couldn't
  /// (no sink, or the extension is behind), and then it never counts as sent.
  @discardableResult
  func send(_ sample: CMSampleBuffer, backgroundID: String, hidesRoom: Bool) -> Bool {
    lock.withLock {
      guard let queue, CMSimpleQueueGetCount(queue) < CMSimpleQueueGetCapacity(queue) else {
        return false
      }
      let numbered =
        ProtectedOutput.sequenceNumberKey.map { key in
          CMSetAttachment(
            sample, key: key, value: NSNumber(value: output.nextSequence),
            attachmentMode: kCMAttachmentMode_ShouldPropagate)
        } != nil
      let element = Unmanaged.passRetained(sample)
      guard CMSimpleQueueEnqueue(queue, element: element.toOpaque()) == noErr else {
        element.release()
        return false
      }
      output.enqueued(backgroundID: backgroundID, hidesRoom: numbered && hidesRoom)
      return true
    }
  }

  /// Whether the extension's latest frame out hid the room for `backgroundID`, lately.
  func hidesRoom(for backgroundID: String) -> Bool {
    lock.withLock { output.hidesRoom(for: backgroundID, now: ProcessInfo.processInfo.systemUptime) }
  }

  private func frameSent(_ sequence: UInt64) {
    lock.withLock { output.sent(sequence, at: ProcessInfo.processInfo.systemUptime) }
  }

  /// The extension's `notifyScheduledOutputChanged`, here as `kCMIOStreamPropertyScheduledOutputNotificationProc`.
  /// The client lives as long as the agent, so the unretained pointer stays good.
  private func listenForSentFrames(_ streamID: CMIOStreamID) {
    var address = Self.address(kCMIOStreamPropertyScheduledOutputNotificationProc)
    var proc = CMIOStreamScheduledOutputNotificationProcAndRefCon(
      scheduledOutputNotificationProc: { sequence, _, refCon in
        guard let refCon else { return }
        Unmanaged<SinkClient>.fromOpaque(refCon).takeUnretainedValue().frameSent(sequence)
      },
      scheduledOutputNotificationRefCon: Unmanaged.passUnretained(self).toOpaque())
    let size = UInt32(MemoryLayout<CMIOStreamScheduledOutputNotificationProcAndRefCon>.size)
    _ = CMIOObjectSetPropertyData(streamID, &address, 0, nil, size, &proc)
  }

  // MARK: CoreMediaIO properties

  private static func address(_ selector: Int) -> CMIOObjectPropertyAddress {
    address(CMIOObjectPropertySelector(selector))
  }

  private static func address(_ selector: CMIOObjectPropertySelector) -> CMIOObjectPropertyAddress {
    CMIOObjectPropertyAddress(
      mSelector: selector, mScope: CMIOObjectPropertyScope(kCMIOObjectPropertyScopeGlobal),
      mElement: CMIOObjectPropertyElement(kCMIOObjectPropertyElementMain))
  }

  private static func ids(_ object: CMIOObjectID, _ selector: Int) -> [CMIOObjectID] {
    var address = address(selector)
    var size: UInt32 = 0
    guard CMIOObjectGetPropertyDataSize(object, &address, 0, nil, &size) == noErr, size > 0 else {
      return []
    }
    var ids = [CMIOObjectID](repeating: 0, count: Int(size) / MemoryLayout<CMIOObjectID>.size)
    var used: UInt32 = 0
    guard CMIOObjectGetPropertyData(object, &address, 0, nil, size, &used, &ids) == noErr else {
      return []
    }
    return ids
  }

  private static func devices() -> [CMIOObjectID] {
    ids(CMIOObjectID(kCMIOObjectSystemObject), kCMIOHardwarePropertyDevices)
  }

  private static func sinkStream(of device: CMIOObjectID) -> CMIOStreamID? {
    let streams = ids(device, kCMIODevicePropertyStreams)
    // A sink is an output stream: frames flow into the device.
    return streams.first { uint32($0, kCMIOStreamPropertyDirection) == 0 }
      ?? (streams.count > 1 ? streams[1] : nil)
  }

  private static func uint32(_ object: CMIOObjectID, _ selector: Int) -> UInt32? {
    var address = address(selector)
    var value: UInt32 = 0
    var used: UInt32 = 0
    let size = UInt32(MemoryLayout<UInt32>.size)
    guard CMIOObjectGetPropertyData(object, &address, 0, nil, size, &used, &value) == noErr else {
      return nil
    }
    return value
  }

  private static func string(_ object: CMIOObjectID, _ selector: Int) -> String? {
    string(object, CMIOObjectPropertySelector(selector))
  }

  private static func string(_ object: CMIOObjectID, _ selector: CMIOObjectPropertySelector)
    -> String?
  {
    var address = address(selector)
    guard CMIOObjectHasProperty(object, &address) else { return nil }
    var value: Unmanaged<CFString>?
    var used: UInt32 = 0
    let size = UInt32(MemoryLayout<Unmanaged<CFString>?>.size)
    guard CMIOObjectGetPropertyData(object, &address, 0, nil, size, &used, &value) == noErr else {
      return nil
    }
    return value?.takeRetainedValue() as String?
  }
}
