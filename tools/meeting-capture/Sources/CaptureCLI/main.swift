import AVFoundation
import AppKit
import CaptureCore
import Foundation
import MeetingAccessibility

/// Anarlog's meeting capture helper. stdout carries framed events and live PCM (see README.md);
/// stdin takes `pause`, `resume` and `stop`. It never prompts, never changes the default input,
/// and never broadens an app-scoped recording to the whole Mac.
@main
struct CaptureCLI {
  static let live = LiveOutput()
  static let protocolVersion = 1
  static let usage =
    "Usage: anarlog-meeting-capture version | calls | record --output DIR --capture-id ID [--scope discover|system|call --pid PID --bundle-id ID --platform P [--call-url URL]] [--microphone NAME_OR_UID] [--microphone-processing automatic|voice|voice-advanced-ducking|plain] [--accessibility on|off] [--seconds N]"

  @MainActor
  static func main() async {
    umask(0o077)
    signal(SIGPIPE, SIG_IGN)
    do { try await run() } catch {
      let code = (error as? CaptureError)?.code ?? "capture_failed"
      live.send(
        EventLine("failed").with("code", code).with("message", error.localizedDescription),
        wait: .now() + 2)
      live.flush()
      FileHandle.standardError.write(Data("\(error.localizedDescription)\n".utf8))
      exit(1)
    }
    live.flush()
  }

  /// Every running app whose call windows the helper can read, and whether it shows a call.
  /// One read per app; nothing is kept, and only a call's own Meet link leaves the reader.
  static func calls() {
    for app in NSWorkspace.shared.runningApplications {
      guard let id = app.bundleIdentifier, MeetingAXAdapter.supports(bundleID: id) else {
        continue
      }
      let reader = MeetingAXReader(processID: app.processIdentifier, bundleID: id)
      guard let call = MeetingAXAdapter.callStatus(bundleID: id, snapshot: reader.snapshot())
      else { continue }
      live.send(
        EventLine("call").with("bundleId", id).with("platform", call.platform)
          .with("status", call.status).with("callUrl", call.callURL),
        wait: .distantFuture)
    }
    live.send(EventLine("done"), wait: .distantFuture)
  }

  struct Scope {
    var tap: CallAudioScope
    var application: NSRunningApplication?
    var platform: String?
    var callURL: String?
  }

  static let platforms: Set<String> = ["google-meet", "zoom", "teams", "webex"]

  /// The scope a recording taps, chosen once at its start from one native read.
  @MainActor
  static func scope(_ options: [String: String]) throws -> Scope {
    switch options["--scope"] ?? "discover" {
    case "system":
      return Scope(tap: .system)
    case "discover":
      let apps = NSWorkspace.shared.runningApplications.compactMap { app in
        app.bundleIdentifier.flatMap { MeetingAXAdapter.supports(bundleID: $0) ? (app, $0) : nil }
      }
      let reads = apps.map { app, id in
        (
          bundleID: id,
          snapshot: MeetingAXReader(processID: app.processIdentifier, bundleID: id).snapshot()
        )
      }
      // Exactly one confirmed call scopes the recording to its app; none or several leave the
      // whole Mac, as Anarlog records without the native engine. The choice is reported.
      guard let call = MeetingAXAdapter.soleCall(reads) else { return Scope(tap: .system) }
      let (app, id) = apps[call.index]
      return Scope(
        tap: .application(pid: app.processIdentifier, bundleID: id), application: app,
        platform: MeetingAXAdapter.platform(bundleID: id), callURL: call.callURL)
    case "call":
      guard let pid = options["--pid"].flatMap(Int32.init), pid > 0,
        let bundleID = options["--bundle-id"], !bundleID.isEmpty,
        let platform = options["--platform"], platforms.contains(platform)
      else { throw CaptureError.invalid("--scope call needs --pid, --bundle-id and --platform") }
      // A re-opened capture keeps exactly the app it confirmed; it never becomes system-wide.
      guard let app = NSRunningApplication(processIdentifier: pid), !app.isTerminated,
        app.bundleIdentifier == bundleID
      else {
        throw CaptureError.refused(
          code: "call_app_unavailable", message: "The call's application is no longer running")
      }
      return Scope(
        tap: .application(pid: pid, bundleID: bundleID), application: app, platform: platform,
        callURL: options["--call-url"])
    default:
      throw CaptureError.invalid("Use --scope discover, system or call")
    }
  }

  @MainActor
  static func run() async throws {
    let arguments = Array(CommandLine.arguments.dropFirst())
    switch arguments.first {
    case "version":
      live.send(EventLine("version").with("protocol", protocolVersion), wait: .distantFuture)
      return
    case "calls":
      calls()
      return
    case "record":
      break
    default:
      throw CaptureError.invalid(usage)
    }
    var options: [String: String] = [:]
    var index = 1
    let allowed: Set<String> = [
      "--output", "--capture-id", "--scope", "--pid", "--bundle-id", "--platform", "--call-url",
      "--microphone", "--microphone-processing", "--accessibility", "--seconds",
    ]
    while index < arguments.count {
      let key = arguments[index]
      guard allowed.contains(key), index + 1 < arguments.count, options[key] == nil else {
        throw CaptureError.invalid("Invalid or repeated option: \(key)")
      }
      options[key] = arguments[index + 1]
      index += 2
    }
    guard let path = options["--output"], !path.isEmpty, let captureID = options["--capture-id"],
      !captureID.isEmpty
    else { throw CaptureError.invalid(usage) }
    // Only `automatic` is qualified; the rest are for device trials.
    guard
      let processing = MicrophoneProcessing(
        rawValue: options["--microphone-processing"] ?? "automatic")
    else { throw CaptureError.invalid(usage) }
    let accessibility = options["--accessibility"] ?? "on"
    guard ["on", "off"].contains(accessibility) else { throw CaptureError.invalid(usage) }
    let duration = Double(options["--seconds"] ?? "43200") ?? .nan
    guard duration.isFinite, duration > 0, duration <= 43_200 else {
      throw CaptureError.invalid("--seconds must be between 0 and 43200")
    }
    let directory = URL(fileURLWithPath: path, isDirectory: true).standardizedFileURL
    let scope = try scope(options)
    let capture = try await NativeCapture.start(
      scope: scope.tap, directory: directory, captureID: captureID,
      microphoneID: options["--microphone"], microphoneProcessing: processing, live: live)
    let output = capture.output
    let pid = scope.application?.processIdentifier
    let bundleID = scope.application?.bundleIdentifier
    try output.started(
      EventLine("started").with("scope", pid == nil ? "system" : "application")
        .with("pid", pid ?? 0).with("bundleId", bundleID).with("platform", scope.platform)
        .with("callUrl", scope.callURL).with("microphoneId", capture.microphone.uniqueID)
        .with("microphoneName", capture.microphone.localizedName)
        .with("micProcessing", capture.microphoneVoiceProcessing ? "voice_processed" : "raw")
        .with("directory", directory.path))
    let observer = pid.flatMap { pid in
      accessibility == "on"
        ? MeetingAXObserver(processID: pid, bundleID: bundleID ?? "") { result, startedAt in
          switch result {
          case .meeting(let meeting):
            output.observeMeeting(meeting, unknownReason: nil, startedAt: startedAt)
          case .unknown(let reason):
            output.observeMeeting(nil, unknownReason: reason, startedAt: startedAt)
          }
        } : nil
    }
    defer { observer?.stop() }

    let (commands, continuation) = AsyncStream<String>.makeStream()
    let input = Task.detached {
      while let line = readLine() { continuation.yield(line.trimmingCharacters(in: .whitespaces)) }
      // Anarlog went away: finish the tracks rather than leave them open.
      continuation.yield("stop")
    }
    signal(SIGINT, SIG_IGN)
    signal(SIGTERM, SIG_IGN)
    let signals = [SIGINT, SIGTERM].map { number in
      let source = DispatchSource.makeSignalSource(signal: number, queue: .global())
      source.setEventHandler { continuation.yield("stop") }
      source.resume()
      return source
    }
    let application = scope.application
    let monitor = Task { @MainActor in
      let deadline = output.origin + duration
      while !Task.isCancelled {
        if output.error != nil {
          continuation.yield("failed")
          return
        }
        if application?.isTerminated == true {
          continuation.yield("app_closed")
          return
        }
        if captureHostTime() >= deadline {
          continuation.yield("duration_limit")
          return
        }
        try? await Task.sleep(nanoseconds: 100_000_000)
      }
    }
    defer {
      monitor.cancel()
      input.cancel()
      for source in signals { source.cancel() }
      continuation.finish()
    }
    var reason = "user_stopped"
    for await command in commands {
      if ["stop", "failed", "app_closed", "duration_limit"].contains(command) {
        reason =
          command == "failed" ? "capture_failed" : command == "stop" ? "user_stopped" : command
        break
      }
      do {
        // `paused` and `resumed` in the journal are the acknowledgements.
        switch command {
        case "pause": try output.pause()
        case "resume": try output.resume()
        default: throw CaptureError.invalid("Expected pause, resume, or stop")
        }
      } catch {
        live.send(EventLine("command-error").with("message", error.localizedDescription))
      }
    }
    _ = try await capture.stop(reason: reason)
    if let error = output.error { throw error }
  }
}
