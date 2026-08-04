import Foundation

final class FloatingBarCommandCoalescer {
  enum Action {
    case show
    case hide
    case update(FloatingBarStatePayload)
  }

  typealias Scheduler = (@escaping () -> Void) -> Void

  private struct SequencedAction {
    let sequence: UInt64
    let action: Action
  }

  private let lock = NSLock()
  private let scheduler: Scheduler
  private let apply: (Action) -> Void
  private var nextSequence: UInt64 = 0
  private var pendingVisibility: SequencedAction?
  private var pendingUpdate: SequencedAction?
  private var drainScheduled = false

  init(
    scheduler: @escaping Scheduler = { work in
      DispatchQueue.main.async(execute: work)
    },
    apply: @escaping (Action) -> Void
  ) {
    self.scheduler = scheduler
    self.apply = apply
  }

  func enqueueShow() {
    enqueue { sequence in
      pendingVisibility = SequencedAction(sequence: sequence, action: .show)
    }
  }

  func enqueueHide() {
    enqueue { sequence in
      pendingVisibility = SequencedAction(sequence: sequence, action: .hide)
    }
  }

  func enqueueUpdate(_ state: FloatingBarStatePayload) {
    enqueue { sequence in
      let previousState: FloatingBarStatePayload?
      if let pendingUpdate, case .update(let state) = pendingUpdate.action {
        previousState = state
      } else {
        previousState = nil
      }
      let state = state.preservingTranscriptBubbles(from: previousState)
      pendingUpdate = SequencedAction(sequence: sequence, action: .update(state))
    }
  }

  private func enqueue(_ merge: (UInt64) -> Void) {
    lock.lock()
    nextSequence &+= 1
    merge(nextSequence)
    let shouldSchedule = !drainScheduled
    drainScheduled = true
    lock.unlock()

    if shouldSchedule {
      scheduleDrain()
    }
  }

  private func scheduleDrain() {
    scheduler { [weak self] in
      self?.drain()
    }
  }

  private func drain() {
    lock.lock()
    let actions = [pendingVisibility, pendingUpdate]
      .compactMap { $0 }
      .sorted { $0.sequence < $1.sequence }
    pendingVisibility = nil
    pendingUpdate = nil
    if actions.isEmpty {
      drainScheduled = false
    }
    lock.unlock()

    guard !actions.isEmpty else { return }

    for action in actions {
      apply(action.action)
    }

    lock.lock()
    let hasPendingActions = pendingVisibility != nil || pendingUpdate != nil
    if !hasPendingActions {
      drainScheduled = false
    }
    lock.unlock()

    if hasPendingActions {
      scheduleDrain()
    }
  }
}

extension FloatingBarStatePayload {
  fileprivate func preservingTranscriptBubbles(
    from previousState: FloatingBarStatePayload?
  ) -> FloatingBarStatePayload {
    guard transcriptBubbles == nil,
      let transcriptBubbles = previousState?.transcriptBubbles
    else {
      return self
    }

    return FloatingBarStatePayload(
      amplitude: amplitude,
      title: title,
      status: status,
      colorScheme: colorScheme,
      opacity: opacity,
      liveCaptionOpacity: liveCaptionOpacity,
      liveCaptionWidth: liveCaptionWidth,
      liveCaptionLineCount: liveCaptionLineCount,
      liveCaptionPosition: liveCaptionPosition,
      liveCaptionMinimized: liveCaptionMinimized,
      liveCaptionToggleVisible: liveCaptionToggleVisible,
      transcriptBubbles: transcriptBubbles)
  }
}
