import Foundation
import ZoomBridgeProtocol

// JSON-lines bridge sidecar: reads `MeetingSdkBridgeCommand` on stdin, writes
// `MeetingSdkBridgeEvent` on stdout. Launched by `plugins/zoom-client` as `char-sidecar-zoom`.

let stdoutLock = NSLock()
func writeLine(_ data: Data) {
  stdoutLock.lock()
  defer { stdoutLock.unlock() }
  FileHandle.standardOutput.write(data)
}

func log(_ message: String) {
  FileHandle.standardError.write((message + "\n").data(using: .utf8)!)
}

final class Sidecar {
  private var driver: MeetingDriver?
  private var stream: BridgeEventStream?
  private var joined = false
  private let decoder = JSONDecoder()

  func handle(line: Data) {
    let command: BridgeCommand
    do {
      command = try decoder.decode(BridgeCommand.self, from: line)
    } catch {
      log("zoom_sidecar_invalid_command \(error)")
      return
    }
    switch command {
    case .start(let start):
      begin(start)
    case .stop(let jobId):
      stop(jobId: jobId)
    }
  }

  private func begin(_ start: BridgeStart) {
    guard driver == nil else {
      log("zoom_sidecar_already_started")
      return
    }
    guard start.protocolVersion == bridgeProtocolVersion,
      start.checkpoint.provider == .zoomMeetingSdk,
      start.checkpoint.meeting.platform == .zoom
    else {
      log("zoom_sidecar_unsupported_start")
      exit(2)
    }
    let stream = BridgeEventStream(checkpoint: start.checkpoint, write: writeLine)
    self.stream = stream
    stream.emit(.ready)

    guard let target = ZoomMeetingURL.parse(start.checkpoint.meeting.url) else {
      finish(.failed, TerminalReason(kind: .invalidMeeting, message: "unrecognized zoom link"))
      return
    }

    let driver = makeMeetingDriver()
    self.driver = driver
    driver.onEvent = { [weak self] event in
      guard let self, let stream = self.stream else { return }
      switch event {
      case .payload(let payload):
        if case .joined = payload { self.joined = true }
        stream.emit(payload)
      case .terminal(let state, let reason):
        self.finish(state, reason)
      }
    }
    driver.join(
      meetingNumber: target.meetingNumber, passcode: target.passcode,
      displayName: start.botName, credentials: MeetingSdkJwt.Credentials.fromEnvironment())
  }

  private func stop(jobId: String) {
    guard let driver else {
      exit(0)
    }
    driver.leave()
    DispatchQueue.main.asyncAfter(deadline: .now() + 4) { [weak self] in
      guard let self else { return }
      self.finish(self.joined ? .completed : .canceled, TerminalReason(kind: .stoppedByRequest))
    }
  }

  private func finish(_ state: BotState, _ reason: TerminalReason) {
    guard let stream else { exit(0) }
    self.stream = nil
    stream.emit(.terminal(BridgeTerminal(state: state, reason: reason)))
    driver?.shutdown()
    driver = nil
    DispatchQueue.main.asyncAfter(deadline: .now() + 0.2) { exit(0) }
  }
}

let sidecar = Sidecar()

let stdin = FileHandle.standardInput
var buffer = Data()
stdin.readabilityHandler = { handle in
  let chunk = handle.availableData
  if chunk.isEmpty {
    DispatchQueue.main.async {
      sidecar.handle(line: Data(#"{"type":"stop","data":{"job_id":""}}"#.utf8))
    }
    handle.readabilityHandler = nil
    return
  }
  buffer.append(chunk)
  while let newline = buffer.firstIndex(of: 0x0A) {
    let line = buffer.subdata(in: buffer.startIndex..<newline)
    buffer.removeSubrange(buffer.startIndex...newline)
    if !line.isEmpty {
      DispatchQueue.main.async { sidecar.handle(line: line) }
    }
  }
}

// The Zoom SDK requires a main-thread run loop (AppKit under the hood).
RunLoop.main.run()
