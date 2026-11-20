import CaptureCore
import Foundation

/// Owns AX IPC on a serial queue, separate from capture callbacks and audio writes.
public final class MeetingAXObserver {
  private let queue = DispatchQueue(label: "so.anarlog.meeting-capture.accessibility")
  private let timer: DispatchSourceTimer

  public init(
    processID: Int32, bundleID: String,
    receive: @escaping (MeetingAXResult, Double) -> Void
  ) {
    let reader = MeetingAXReader(processID: processID, bundleID: bundleID)
    timer = DispatchSource.makeTimerSource(queue: queue)
    timer.schedule(deadline: .now(), repeating: .seconds(1), leeway: .milliseconds(100))
    timer.setEventHandler {
      let startedAt = captureHostTime()
      // Unsupported applications must not trigger a broad UI traversal.
      let result: MeetingAXResult =
        MeetingAXAdapter.supports(bundleID: bundleID)
        ? MeetingAXAdapter.project(bundleID: bundleID, snapshot: reader.snapshot())
        : .unknown("unsupported_application")
      receive(result, startedAt)
    }
    timer.resume()
  }

  /// Call outside the observer callback. No observation can arrive after this returns.
  public func stop() {
    timer.cancel()
    queue.sync {}
  }

  deinit { timer.cancel() }
}
