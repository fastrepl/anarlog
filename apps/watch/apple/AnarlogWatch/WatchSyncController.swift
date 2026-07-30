import Combine
import Foundation
import WatchConnectivity

struct AccountIdentity: Codable, Equatable {
  let userId: String
  let email: String?
}

private struct PendingRecording: Codable, Equatable {
  let id: String
  let url: URL
  let recordedAt: Date
  let accountUserId: String
}

final class WatchSyncController: NSObject, ObservableObject {
  @Published private(set) var account: AccountIdentity?
  @Published private(set) var isPhoneReachable = false
  @Published private(set) var activationState = WCSessionActivationState.notActivated
  @Published private(set) var pendingTransferCount = 0

  private static let accountDefaultsKey = "watch-connectivity-account"
  private static let pendingRecordingsDefaultsKey =
    "watch-connectivity-pending-recordings"
  private static let acknowledgementTimeout: TimeInterval = 60

  private let defaults = UserDefaults.standard
  private let iso8601Formatter = ISO8601DateFormatter()
  private var pendingRecordings: [PendingRecording] = []
  private var queuedRecordingIds: Set<String> = []
  private var acknowledgementWorkItems: [String: DispatchWorkItem] = [:]
  private var retryDelay: TimeInterval = 2
  private var retryWorkItem: DispatchWorkItem?

  override init() {
    if let data = UserDefaults.standard.data(forKey: Self.accountDefaultsKey) {
      account = try? JSONDecoder().decode(AccountIdentity.self, from: data)
    }
    if let data = UserDefaults.standard.data(
      forKey: Self.pendingRecordingsDefaultsKey
    ) {
      pendingRecordings =
        (try? JSONDecoder().decode([PendingRecording].self, from: data)) ?? []
    }
    pendingTransferCount = pendingRecordings.count

    #if DEBUG
      if ProcessInfo.processInfo.arguments.contains("-demo-account") {
        account = AccountIdentity(
          userId: "preview-account",
          email: "jane@anarlog.so"
        )
      }
    #endif

    super.init()
    activate()
  }

  var statusText: String {
    switch activationState {
    case .activated where isPhoneReachable:
      return "iPhone connected"
    case .activated:
      return "Syncs when your iPhone is nearby"
    case .inactive:
      return "Waiting for iPhone"
    case .notActivated:
      return "Connecting…"
    @unknown default:
      return "Waiting for iPhone"
    }
  }

  func refreshAccount() {
    guard WCSession.isSupported() else {
      return
    }

    let session = WCSession.default
    applyAccountContext(session.receivedApplicationContext)
    updateState(from: session)

    guard session.activationState == .activated else {
      session.activate()
      return
    }

    guard session.isReachable else {
      return
    }

    session.sendMessage(
      ["schema_version": 1, "kind": "account_request"],
      replyHandler: { [weak self] response in
        DispatchQueue.main.async {
          self?.applyAccountContext(response)
        }
      },
      errorHandler: nil
    )
  }

  func enqueueRecording(
    url: URL,
    recordedAt: Date,
    accountUserId: String
  ) {
    let recording = PendingRecording(
      id: url.deletingPathExtension().lastPathComponent,
      url: url,
      recordedAt: recordedAt,
      accountUserId: accountUserId
    )
    if !pendingRecordings.contains(where: { $0.id == recording.id }) {
      pendingRecordings.append(recording)
      persistPendingRecordings()
    }

    flushPendingRecordings()
  }

  func syncNow() {
    refreshAccount()
    for workItem in acknowledgementWorkItems.values {
      workItem.cancel()
    }
    acknowledgementWorkItems.removeAll()
    queuedRecordingIds.removeAll()
    flushPendingRecordings()
  }

  private func activate() {
    guard WCSession.isSupported() else {
      return
    }

    let session = WCSession.default
    session.delegate = self
    session.activate()
  }

  private func flushPendingRecordings() {
    pendingRecordings.removeAll {
      !FileManager.default.fileExists(atPath: $0.url.path)
    }
    queuedRecordingIds.formIntersection(pendingRecordings.map(\.id))
    persistPendingRecordings()

    guard WCSession.isSupported() else {
      return
    }

    let session = WCSession.default
    guard session.activationState == .activated else {
      session.activate()
      return
    }

    let outstandingIds = Set(
      session.outstandingFileTransfers.compactMap {
        $0.file.metadata?["id"] as? String
      }
    )
    for recording in pendingRecordings
    where
      !outstandingIds.contains(recording.id)
      && !queuedRecordingIds.contains(recording.id)
    {
      queuedRecordingIds.insert(recording.id)
      session.transferFile(
        recording.url,
        metadata: [
          "id": recording.id,
          "recorded_at": iso8601Formatter.string(from: recording.recordedAt),
          "account_user_id": recording.accountUserId,
        ]
      )
    }
    updateState(from: session)
  }

  private func applyAccountContext(_ context: [String: Any]) {
    guard context["kind"] as? String == "account" else {
      return
    }

    let signedIn = context["signed_in"] as? Bool ?? false
    guard
      signedIn,
      let userId = context["user_id"] as? String,
      !userId.isEmpty
    else {
      account = nil
      defaults.removeObject(forKey: Self.accountDefaultsKey)
      return
    }

    let nextAccount = AccountIdentity(
      userId: userId,
      email: context["email"] as? String
    )
    account = nextAccount
    if let data = try? JSONEncoder().encode(nextAccount) {
      defaults.set(data, forKey: Self.accountDefaultsKey)
    }
  }

  private func updateState(from session: WCSession) {
    activationState = session.activationState
    isPhoneReachable =
      session.activationState == .activated
      && session.isReachable
    pendingTransferCount = pendingRecordings.count
  }

  private func persistPendingRecordings() {
    pendingTransferCount = pendingRecordings.count
    if let data = try? JSONEncoder().encode(pendingRecordings) {
      defaults.set(data, forKey: Self.pendingRecordingsDefaultsKey)
    }
  }

  private func acknowledgeImportedRecording(id: String) {
    guard let index = pendingRecordings.firstIndex(where: { $0.id == id }) else {
      return
    }

    let recording = pendingRecordings.remove(at: index)
    acknowledgementWorkItems.removeValue(forKey: id)?.cancel()
    queuedRecordingIds.remove(id)
    persistPendingRecordings()
    try? FileManager.default.removeItem(at: recording.url)
    retryDelay = 2
    flushPendingRecordings()
  }

  private func retryRecording(id: String) {
    acknowledgementWorkItems.removeValue(forKey: id)?.cancel()
    guard queuedRecordingIds.remove(id) != nil else {
      return
    }
    scheduleRetry()
  }

  private func scheduleAcknowledgementRetry(id: String) {
    guard
      pendingRecordings.contains(where: { $0.id == id }),
      acknowledgementWorkItems[id] == nil
    else {
      return
    }

    let workItem = DispatchWorkItem { [weak self] in
      guard let self else {
        return
      }
      acknowledgementWorkItems.removeValue(forKey: id)
      guard pendingRecordings.contains(where: { $0.id == id }) else {
        return
      }
      queuedRecordingIds.remove(id)
      flushPendingRecordings()
    }
    acknowledgementWorkItems[id] = workItem
    DispatchQueue.main.asyncAfter(
      deadline: .now() + Self.acknowledgementTimeout,
      execute: workItem
    )
  }

  private func scheduleRetry() {
    guard retryWorkItem == nil else {
      return
    }

    let delay = retryDelay
    retryDelay = min(retryDelay * 2, 60)
    let workItem = DispatchWorkItem { [weak self] in
      guard let self else {
        return
      }
      retryWorkItem = nil
      flushPendingRecordings()
    }
    retryWorkItem = workItem
    DispatchQueue.main.asyncAfter(deadline: .now() + delay, execute: workItem)
  }
}

extension WatchSyncController: WCSessionDelegate {
  func session(
    _ session: WCSession,
    activationDidCompleteWith activationState: WCSessionActivationState,
    error: Error?
  ) {
    DispatchQueue.main.async { [weak self] in
      self?.updateState(from: session)
      if error == nil && activationState == .activated {
        self?.applyAccountContext(session.receivedApplicationContext)
        self?.refreshAccount()
        self?.flushPendingRecordings()
      }
    }
  }

  func sessionReachabilityDidChange(_ session: WCSession) {
    DispatchQueue.main.async { [weak self] in
      self?.updateState(from: session)
      if session.isReachable {
        self?.refreshAccount()
        self?.flushPendingRecordings()
      }
    }
  }

  func session(
    _ session: WCSession,
    didReceiveApplicationContext applicationContext: [String: Any]
  ) {
    DispatchQueue.main.async { [weak self] in
      self?.applyAccountContext(applicationContext)
      self?.updateState(from: session)
    }
  }

  func session(_ session: WCSession, didReceiveMessage message: [String: Any]) {
    DispatchQueue.main.async { [weak self] in
      self?.applyAccountContext(message)
      self?.updateState(from: session)
    }
  }

  func session(
    _ session: WCSession,
    didFinish fileTransfer: WCSessionFileTransfer,
    error: Error?
  ) {
    DispatchQueue.main.async { [weak self] in
      guard let self else {
        return
      }

      if error != nil {
        let id =
          fileTransfer.file.metadata?["id"] as? String
          ?? fileTransfer.file.fileURL.deletingPathExtension()
          .lastPathComponent
        self.queuedRecordingIds.remove(id)
        self.acknowledgementWorkItems.removeValue(forKey: id)?.cancel()
        self.scheduleRetry()
      } else if let id = fileTransfer.file.metadata?["id"] as? String {
        self.scheduleAcknowledgementRetry(id: id)
      }
      self.updateState(from: session)
    }
  }

  func session(_ session: WCSession, didReceiveUserInfo userInfo: [String: Any]) {
    guard
      let kind = userInfo["kind"] as? String,
      let id = userInfo["id"] as? String
    else {
      return
    }

    DispatchQueue.main.async { [weak self] in
      switch kind {
      case "recording_imported":
        self?.acknowledgeImportedRecording(id: id)
      case "recording_receive_failed":
        self?.retryRecording(id: id)
      default:
        break
      }
    }
  }
}
