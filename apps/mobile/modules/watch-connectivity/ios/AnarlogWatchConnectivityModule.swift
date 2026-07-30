import ExpoModulesCore
import Foundation
import UIKit
import WatchConnectivity

public class AnarlogWatchConnectivityModule: Module {
  public func definition() -> ModuleDefinition {
    Name("AnarlogWatchConnectivity")

    Events("onRecordingReceived", "onStateChanged")

    OnCreate {
      WatchConnectivityService.shared.attach(module: self)
    }

    OnDestroy {
      WatchConnectivityService.shared.detach(module: self)
    }

    Function("updateAccount") { (userId: String?, email: String?) in
      WatchConnectivityService.shared.updateAccount(
        userId: userId,
        email: email
      )
    }

    Function("getState") {
      WatchConnectivityService.shared.statePayload()
    }

    Function("getPendingRecordings") {
      WatchConnectivityService.shared.pendingRecordingPayloads()
    }

    Function("markRecordingImported") { (id: String) in
      WatchConnectivityService.shared.markRecordingImported(id: id)
    }
  }

  fileprivate func emitRecording(_ recording: PendingWatchRecording) {
    sendEvent("onRecordingReceived", recording.payload)
  }

  fileprivate func emitState(_ state: [String: Any]) {
    sendEvent("onStateChanged", state)
  }
}

public class AnarlogWatchConnectivityAppDelegateSubscriber:
  ExpoAppDelegateSubscriber
{
  public func application(
    _ application: UIApplication,
    didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]? = nil
  ) -> Bool {
    WatchConnectivityService.shared.activate()
    return true
  }
}

private struct PendingWatchRecording: Codable {
  let id: String
  let uri: String
  let filename: String
  let recordedAt: String
  let accountUserId: String

  var payload: [String: Any] {
    [
      "id": id,
      "uri": uri,
      "filename": filename,
      "recordedAt": recordedAt,
      "accountUserId": accountUserId,
    ]
  }
}

private final class WatchConnectivityService: NSObject, WCSessionDelegate {
  static let shared = WatchConnectivityService()

  private let accountUserIdKey = "watch-connectivity-account-user-id"
  private let accountEmailKey = "watch-connectivity-account-email"
  private let pendingRecordingsKey = "watch-connectivity-pending-recordings"
  private let pendingAcknowledgementsKey =
    "watch-connectivity-pending-acknowledgements"
  private let defaults = UserDefaults.standard
  private let recordingsQueue = DispatchQueue(
    label: "so.anarlog.watch-connectivity.recordings"
  )
  private weak var module: AnarlogWatchConnectivityModule?

  private override init() {
    super.init()
  }

  func attach(module: AnarlogWatchConnectivityModule) {
    self.module = module
    activate()
    emitState()
  }

  func detach(module: AnarlogWatchConnectivityModule) {
    if self.module === module {
      self.module = nil
    }
  }

  func activate() {
    guard WCSession.isSupported() else {
      return
    }

    let session = WCSession.default
    session.delegate = self
    session.activate()
  }

  func updateAccount(userId: String?, email: String?) {
    if let userId, !userId.isEmpty {
      defaults.set(userId, forKey: accountUserIdKey)
      defaults.set(email, forKey: accountEmailKey)
    } else {
      defaults.removeObject(forKey: accountUserIdKey)
      defaults.removeObject(forKey: accountEmailKey)
    }

    pushAccountContext()
  }

  func statePayload() -> [String: Any] {
    guard WCSession.isSupported() else {
      return [
        "supported": false,
        "activationState": "unsupported",
        "paired": false,
        "watchAppInstalled": false,
        "reachable": false,
      ]
    }

    let session = WCSession.default
    return [
      "supported": true,
      "activationState": activationStateName(session.activationState),
      "paired": session.activationState == .activated && session.isPaired,
      "watchAppInstalled": session.activationState == .activated
        && session.isWatchAppInstalled,
      "reachable": session.activationState == .activated
        && session.isReachable,
    ]
  }

  func pendingRecordingPayloads() -> [[String: Any]] {
    guard let userId = defaults.string(forKey: accountUserIdKey) else {
      return []
    }
    return recordingsQueue.sync {
      pendingRecordings()
        .filter { $0.accountUserId == userId }
        .map(\.payload)
    }
  }

  func markRecordingImported(id: String) {
    let imported = recordingsQueue.sync {
      var recordings = pendingRecordings()
      guard let index = recordings.firstIndex(where: { $0.id == id }) else {
        return false
      }

      let recording = recordings.remove(at: index)
      var acknowledgements = pendingAcknowledgements()
      acknowledgements[id] = "recording_imported"
      persistAcknowledgements(acknowledgements)
      if let url = URL(string: recording.uri) {
        try? FileManager.default.removeItem(at: url)
      }
      persist(recordings)
      return true
    }

    if imported {
      flushAcknowledgements()
    }
  }

  private func accountContext() -> [String: Any] {
    guard let userId = defaults.string(forKey: accountUserIdKey) else {
      return [
        "schema_version": 1,
        "kind": "account",
        "signed_in": false,
      ]
    }

    var context: [String: Any] = [
      "schema_version": 1,
      "kind": "account",
      "signed_in": true,
      "user_id": userId,
    ]
    if let email = defaults.string(forKey: accountEmailKey), !email.isEmpty {
      context["email"] = email
    }
    return context
  }

  private func pushAccountContext() {
    guard WCSession.isSupported() else {
      return
    }

    let session = WCSession.default
    guard session.activationState == .activated else {
      return
    }

    try? session.updateApplicationContext(accountContext())
    flushAcknowledgements()
    if session.isReachable {
      session.sendMessage(
        accountContext(),
        replyHandler: nil,
        errorHandler: nil
      )
    }
    emitState()
  }

  private func receive(_ file: WCSessionFile, from session: WCSession) {
    let metadata = file.metadata ?? [:]
    guard let accountUserId = metadata["account_user_id"] as? String else {
      return
    }
    let id = metadata["id"] as? String ?? UUID().uuidString
    let recordedAt =
      metadata["recorded_at"] as? String
      ?? ISO8601DateFormatter().string(from: Date())
    let fileExtension =
      file.fileURL.pathExtension.isEmpty
      ? "m4a"
      : file.fileURL.pathExtension
    let filename = "\(id).\(fileExtension)"

    let recording: PendingWatchRecording
    do {
      recording = try recordingsQueue.sync {
        let directory = try watchInboxDirectory()
        let destination = directory.appendingPathComponent(filename)
        if !FileManager.default.fileExists(atPath: destination.path) {
          try FileManager.default.moveItem(at: file.fileURL, to: destination)
        }

        var recordings = pendingRecordings()
        let recording = PendingWatchRecording(
          id: id,
          uri: destination.absoluteString,
          filename: filename,
          recordedAt: recordedAt,
          accountUserId: accountUserId
        )
        recordings.removeAll(where: { $0.id == id })
        recordings.append(recording)
        persist(recordings)
        return recording
      }
    } catch {
      queueAcknowledgement(kind: "recording_receive_failed", id: id)
      return
    }

    DispatchQueue.main.async { [weak self] in
      guard let self else {
        return
      }
      guard
        self.defaults.string(forKey: self.accountUserIdKey)
          == recording.accountUserId
      else {
        return
      }
      self.module?.emitRecording(recording)
    }
  }

  private func watchInboxDirectory() throws -> URL {
    let directory = FileManager.default
      .urls(for: .documentDirectory, in: .userDomainMask)[0]
      .appendingPathComponent("watch-inbox", isDirectory: true)
    try FileManager.default.createDirectory(
      at: directory,
      withIntermediateDirectories: true
    )
    return directory
  }

  private func pendingRecordings() -> [PendingWatchRecording] {
    guard let data = defaults.data(forKey: pendingRecordingsKey) else {
      return []
    }
    return (try? JSONDecoder().decode([PendingWatchRecording].self, from: data))
      ?? []
  }

  private func persist(_ recordings: [PendingWatchRecording]) {
    guard let data = try? JSONEncoder().encode(recordings) else {
      return
    }
    defaults.set(data, forKey: pendingRecordingsKey)
  }

  private func queueAcknowledgement(kind: String, id: String) {
    recordingsQueue.sync {
      var acknowledgements = pendingAcknowledgements()
      acknowledgements[id] = kind
      persistAcknowledgements(acknowledgements)
    }
    flushAcknowledgements()
  }

  private func flushAcknowledgements() {
    guard WCSession.isSupported() else {
      return
    }

    let session = WCSession.default
    guard session.activationState == .activated else {
      return
    }

    let acknowledgements = recordingsQueue.sync {
      pendingAcknowledgements()
    }
    for (id, kind) in acknowledgements {
      session.transferUserInfo([
        "schema_version": 1,
        "kind": kind,
        "id": id,
      ])
    }

    recordingsQueue.sync {
      var pending = pendingAcknowledgements()
      for (id, kind) in acknowledgements where pending[id] == kind {
        pending.removeValue(forKey: id)
      }
      persistAcknowledgements(pending)
    }
  }

  private func pendingAcknowledgements() -> [String: String] {
    guard let data = defaults.data(forKey: pendingAcknowledgementsKey) else {
      return [:]
    }
    return (try? JSONDecoder().decode([String: String].self, from: data)) ?? [:]
  }

  private func persistAcknowledgements(
    _ acknowledgements: [String: String]
  ) {
    guard let data = try? JSONEncoder().encode(acknowledgements) else {
      return
    }
    defaults.set(data, forKey: pendingAcknowledgementsKey)
  }

  private func activationStateName(
    _ state: WCSessionActivationState
  ) -> String {
    switch state {
    case .notActivated:
      return "not_activated"
    case .inactive:
      return "inactive"
    case .activated:
      return "activated"
    @unknown default:
      return "unknown"
    }
  }

  private func emitState() {
    let state = statePayload()
    DispatchQueue.main.async { [weak self] in
      self?.module?.emitState(state)
    }
  }

  func session(
    _ session: WCSession,
    activationDidCompleteWith activationState: WCSessionActivationState,
    error: Error?
  ) {
    if error == nil && activationState == .activated {
      pushAccountContext()
    }
    emitState()
  }

  func sessionDidBecomeInactive(_ session: WCSession) {
    emitState()
  }

  func sessionDidDeactivate(_ session: WCSession) {
    session.activate()
    emitState()
  }

  func sessionWatchStateDidChange(_ session: WCSession) {
    pushAccountContext()
    emitState()
  }

  func session(
    _ session: WCSession,
    didReceiveMessage message: [String: Any],
    replyHandler: @escaping ([String: Any]) -> Void
  ) {
    if message["kind"] as? String == "account_request" {
      replyHandler(accountContext())
    } else {
      replyHandler([:])
    }
  }

  func session(_ session: WCSession, didReceive file: WCSessionFile) {
    receive(file, from: session)
  }
}
