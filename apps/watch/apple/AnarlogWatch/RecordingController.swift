import AVFAudio
import Combine
import Foundation

struct CompletedRecording: Equatable {
  let url: URL
  let recordedAt: Date
}

@MainActor
final class RecordingController: ObservableObject {
  @Published private(set) var isRecording = false
  @Published private(set) var levels = Array(repeating: CGFloat(0.12), count: 28)
  @Published private(set) var errorMessage: String?
  @Published private(set) var lastCompletedRecording: CompletedRecording?

  private let audioSession = AVAudioSession.sharedInstance()
  private var startTask: Task<Void, Never>?
  private var meterTimer: Timer?
  private var audioRecorder: AVAudioRecorder?
  private var currentRecordingURL: URL?
  private var recordingStartedAt: Date?

  func toggle() {
    if isRecording {
      stop()
      return
    }

    guard startTask == nil else {
      return
    }
    startTask = Task { [weak self] in
      guard let self else {
        return
      }
      await start()
      startTask = nil
    }
  }

  func dismissError() {
    errorMessage = nil
  }

  func stopIfNeeded() {
    startTask?.cancel()
    if isRecording {
      stop()
    }
  }

  private func start() async {
    let hasMicrophonePermission = await requestMicrophonePermission()
    guard !Task.isCancelled else {
      return
    }
    guard hasMicrophonePermission else {
      errorMessage = "Allow microphone access in Settings to record from your watch."
      return
    }

    var recordingURL: URL?

    do {
      try audioSession.setCategory(.record, mode: .default)
      try await activateAudioSession()
      try Task.checkCancellation()

      let url = try nextRecordingURL()
      recordingURL = url
      let recorder = try AVAudioRecorder(
        url: url,
        settings: [
          AVFormatIDKey: Int(kAudioFormatMPEG4AAC),
          AVSampleRateKey: 16_000,
          AVNumberOfChannelsKey: 1,
          AVEncoderBitRateKey: 64_000,
          AVEncoderAudioQualityKey: AVAudioQuality.high.rawValue,
        ]
      )
      recorder.isMeteringEnabled = true
      audioRecorder = recorder

      guard recorder.record() else {
        throw RecordingError.couldNotStart
      }
      try Task.checkCancellation()

      currentRecordingURL = url
      recordingStartedAt = Date()
      isRecording = true
      startMetering()
    } catch {
      audioRecorder?.stop()
      audioRecorder = nil
      currentRecordingURL = nil
      recordingStartedAt = nil
      if let recordingURL {
        try? FileManager.default.removeItem(at: recordingURL)
      }
      try? audioSession.setActive(false, options: .notifyOthersOnDeactivation)
      if !(error is CancellationError) {
        errorMessage = "Recording could not start. Try again in a moment."
      }
    }
  }

  private func stop() {
    let completedRecording: CompletedRecording? =
      if isRecording,
        let currentRecordingURL,
        let recordingStartedAt
      {
        CompletedRecording(
          url: currentRecordingURL,
          recordedAt: recordingStartedAt
        )
      } else {
        nil
      }

    meterTimer?.invalidate()
    meterTimer = nil
    audioRecorder?.stop()
    audioRecorder = nil
    currentRecordingURL = nil
    recordingStartedAt = nil
    isRecording = false
    levels = Array(repeating: 0.12, count: levels.count)
    try? audioSession.setActive(false, options: .notifyOthersOnDeactivation)

    if let completedRecording,
      FileManager.default.fileExists(atPath: completedRecording.url.path)
    {
      lastCompletedRecording = completedRecording
    }
  }

  private func requestMicrophonePermission() async -> Bool {
    switch AVAudioApplication.shared.recordPermission {
    case .granted:
      return true
    case .denied:
      return false
    case .undetermined:
      return await withCheckedContinuation { continuation in
        AVAudioApplication.requestRecordPermission { granted in
          continuation.resume(returning: granted)
        }
      }
    @unknown default:
      return false
    }
  }

  private func activateAudioSession() async throws {
    try await withCheckedThrowingContinuation { continuation in
      audioSession.activate(options: []) { activated, error in
        if activated {
          continuation.resume()
        } else {
          continuation.resume(throwing: error ?? RecordingError.couldNotActivateAudio)
        }
      }
    }
  }

  private func nextRecordingURL() throws -> URL {
    let recordingsDirectory = FileManager.default
      .urls(for: .documentDirectory, in: .userDomainMask)[0]
      .appendingPathComponent("Recordings", isDirectory: true)
    try FileManager.default.createDirectory(
      at: recordingsDirectory,
      withIntermediateDirectories: true
    )

    return
      recordingsDirectory
      .appendingPathComponent(UUID().uuidString)
      .appendingPathExtension("m4a")
  }

  private func startMetering() {
    levels = Array(repeating: 0.12, count: levels.count)
    meterTimer?.invalidate()
    meterTimer = Timer.scheduledTimer(withTimeInterval: 0.08, repeats: true) {
      [weak self] _ in
      Task { @MainActor [weak self] in
        self?.updateMeter()
      }
    }
  }

  private func updateMeter() {
    guard let audioRecorder, audioRecorder.isRecording else {
      stop()
      return
    }

    audioRecorder.updateMeters()
    let average = normalizedPower(audioRecorder.averagePower(forChannel: 0))
    let peak = normalizedPower(audioRecorder.peakPower(forChannel: 0))
    let level = max(average, peak * 0.75)

    levels.removeFirst()
    levels.append(level)
  }

  private func normalizedPower(_ decibels: Float) -> CGFloat {
    let floor: Float = -55
    let clamped = max(floor, min(0, decibels))
    let linearAmplitude = pow(10, Double(clamped / 20))
    return max(0.08, min(1, CGFloat(linearAmplitude * 3.5)))
  }
}

private enum RecordingError: Error {
  case couldNotActivateAudio
  case couldNotStart
}
