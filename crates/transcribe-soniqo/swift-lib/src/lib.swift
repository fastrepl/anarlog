import AudioCommon
import Foundation
import OmnilingualASR
import ParakeetASR
import ParakeetStreamingASR
import SwiftRs

private enum SoniqoBridgeError: LocalizedError {
  case message(String)

  var errorDescription: String? {
    switch self {
    case .message(let message):
      return message
    }
  }
}

private let soniqoFileTranscriptionSampleRate = 16_000
private let parakeetBatchMinimumChunkSeconds = 5.0
private let parakeetBatchMaximumChunkSeconds = 19.5

private enum SpeechModelKind: String, CaseIterable {
  case parakeetStreaming = "soniqo-parakeet-streaming"
  case parakeetBatch = "soniqo-parakeet-batch"
  case omnilingual = "soniqo-omnilingual"
  case qwen3Small = "soniqo-qwen3-small"
  case qwen3Large = "soniqo-qwen3-large"

  static func resolve(_ identifier: String) -> Self? {
    Self(rawValue: identifier) ?? Self.allCases.first(where: { $0.repo == identifier })
  }

  var label: String {
    switch self {
    case .parakeetStreaming:
      return "Soniqo Parakeet Streaming"
    case .parakeetBatch:
      return "Soniqo Parakeet Batch"
    case .omnilingual:
      return "Soniqo Omnilingual"
    case .qwen3Small:
      return "Soniqo Qwen3 0.6B"
    case .qwen3Large:
      return "Soniqo Qwen3 1.7B"
    }
  }

  var repo: String {
    switch self {
    case .parakeetStreaming:
      return "aufklarer/Parakeet-EOU-120M-CoreML-INT8"
    case .parakeetBatch:
      return "aufklarer/Parakeet-TDT-v3-CoreML-INT8"
    case .omnilingual:
      return "aufklarer/Omnilingual-ASR-CTC-300M-CoreML-INT8-10s"
    case .qwen3Small:
      return "aufklarer/Qwen3-ASR-0.6B-MLX-4bit"
    case .qwen3Large:
      return "aufklarer/Qwen3-ASR-1.7B-MLX-8bit"
    }
  }

  var isStreamingCapable: Bool {
    self == .parakeetStreaming
  }

  var fileTranscriptionChunkSeconds: Double? {
    switch self {
    case .parakeetStreaming, .qwen3Small, .qwen3Large:
      return nil
    case .parakeetBatch:
      return parakeetBatchMaximumChunkSeconds
    case .omnilingual:
      return 35
    }
  }

  var minimumFileTranscriptionChunkSeconds: Double? {
    switch self {
    case .parakeetBatch:
      return parakeetBatchMinimumChunkSeconds
    case .parakeetStreaming, .omnilingual, .qwen3Small, .qwen3Large:
      return nil
    }
  }

  var maximumFileTranscriptionChunkSeconds: Double? {
    switch self {
    case .parakeetBatch:
      return parakeetBatchMaximumChunkSeconds
    case .parakeetStreaming, .omnilingual, .qwen3Small, .qwen3Large:
      return nil
    }
  }

  func cacheDirectoryURL() throws -> URL {
    try HuggingFaceDownloader.getCacheDirectory(for: repo)
  }

  func cacheDirectoryPath() -> String {
    (try? cacheDirectoryURL().path) ?? ""
  }

  func filesReady() -> Bool {
    guard let directory = try? cacheDirectoryURL() else {
      return false
    }

    switch self {
    case .parakeetStreaming, .parakeetBatch:
      return Self.regularFileExists(at: directory.appendingPathComponent("config.json"))
        && Self.regularFileExists(at: directory.appendingPathComponent("vocab.json"))
        && Self.compiledCoreMLModelReady(at: directory.appendingPathComponent("encoder.mlmodelc"))
        && Self.compiledCoreMLModelReady(at: directory.appendingPathComponent("decoder.mlmodelc"))
        && Self.compiledCoreMLModelReady(at: directory.appendingPathComponent("joint.mlmodelc"))
    case .omnilingual:
      return Self.regularFileExists(at: directory.appendingPathComponent("config.json"))
        && Self.regularFileExists(at: directory.appendingPathComponent("tokenizer.model"))
        && Self.directoryContainsRegularFile(
          at: directory.appendingPathComponent("omnilingual-ctc-300m-int8.mlpackage")
        )
    case .qwen3Small, .qwen3Large:
      return Self.regularFileExists(at: directory.appendingPathComponent("vocab.json"))
        && Self.regularFileExists(at: directory.appendingPathComponent("merges.txt"))
        && Self.regularFileExists(at: directory.appendingPathComponent("tokenizer_config.json"))
        && Self.directoryContainsFile(withExtension: "safetensors", in: directory)
    }
  }

  func load(progressHandler: ((Double, String) -> Void)?) async throws -> LoadedSpeechModel {
    let offlineMode = filesReady()

    switch self {
    case .parakeetStreaming:
      return .streaming(
        try await ParakeetStreamingASRModel.fromPretrained(
          modelId: repo,
          progressHandler: progressHandler
        )
      )
    case .parakeetBatch:
      return .parakeetBatch(
        try await ParakeetASRModel.fromPretrained(
          modelId: repo,
          offlineMode: offlineMode,
          progressHandler: progressHandler
        )
      )
    case .omnilingual:
      return .omnilingual(
        try await OmnilingualASRModel.fromPretrained(
          modelId: repo,
          offlineMode: offlineMode,
          progressHandler: progressHandler
        )
      )
    case .qwen3Small, .qwen3Large:
      throw SoniqoBridgeError.message("\(label) requires macOS 15 or newer.")
    }
  }

  private static func regularFileExists(at url: URL) -> Bool {
    var isDirectory = ObjCBool(false)
    return FileManager.default.fileExists(atPath: url.path, isDirectory: &isDirectory)
      && !isDirectory.boolValue
  }

  private static func compiledCoreMLModelReady(at directory: URL) -> Bool {
    var isDirectory = ObjCBool(false)
    guard FileManager.default.fileExists(atPath: directory.path, isDirectory: &isDirectory),
      isDirectory.boolValue
    else {
      return false
    }

    return regularFileExists(at: directory.appendingPathComponent("model.mil"))
      && directoryContainsRegularFile(at: directory.appendingPathComponent("weights"))
  }

  private static func directoryContainsFile(withExtension pathExtension: String, in directory: URL)
    -> Bool
  {
    guard
      let contents = try? FileManager.default.contentsOfDirectory(
        at: directory,
        includingPropertiesForKeys: [.isRegularFileKey]
      )
    else {
      return false
    }

    return contents.contains { candidate in
      guard
        candidate.pathExtension == pathExtension,
        let values = try? candidate.resourceValues(forKeys: [.isRegularFileKey])
      else {
        return false
      }

      return values.isRegularFile == true
    }
  }

  private static func directoryContainsRegularFile(at directory: URL) -> Bool {
    guard
      let enumerator = FileManager.default.enumerator(
        at: directory,
        includingPropertiesForKeys: [.isRegularFileKey],
        options: [.skipsHiddenFiles]
      )
    else {
      return false
    }

    for case let candidate as URL in enumerator {
      guard let values = try? candidate.resourceValues(forKeys: [.isRegularFileKey]) else {
        continue
      }

      if values.isRegularFile == true {
        return true
      }
    }

    return false
  }
}

private enum LoadedSpeechModel {
  case streaming(ParakeetStreamingASRModel)
  case parakeetBatch(ParakeetASRModel)
  case omnilingual(OmnilingualASRModel)

  func asStreamingModel() throws -> ParakeetStreamingASRModel {
    guard case .streaming(let model) = self else {
      throw SoniqoBridgeError.message(
        "The selected Soniqo model does not support realtime transcription.")
    }

    return model
  }

  func transcribe(audio: [Float], sampleRate: Int, language: String?) throws -> String {
    let normalizedLanguage = language?.trimmingCharacters(in: .whitespacesAndNewlines)
    let languageHint = (normalizedLanguage?.isEmpty == false) ? normalizedLanguage : nil

    switch self {
    case .streaming(let model):
      return try model.transcribeAudio(audio, sampleRate: sampleRate)
    case .parakeetBatch(let model):
      return try model.transcribeAudio(audio, sampleRate: sampleRate, language: languageHint)
    case .omnilingual(let model):
      return try model.transcribeAudio(audio, sampleRate: sampleRate)
    }
  }
}

private enum TranscriptSource: String, Codable, CaseIterable {
  case microphone
  case system
}

private struct ModelDownloadPayload: Codable {
  var status: String
  var currentFile: String?
  var progressPercent: Int?
  var localPath: String
  var error: String?
}

private struct FileTranscriptionPayload: Codable {
  var text: String
  var durationSeconds: Double
  var error: String?
}

private struct LivePartialPayload: Codable {
  var source: String
  var text: String
  var isFinal: Bool
}

private struct LiveAppendPayload: Codable {
  var partials: [LivePartialPayload]
  var error: String?
}

private struct StatusPayload: Codable {
  var running: Bool
  var error: String?
}

private func encodeJSON<T: Encodable>(_ value: T) -> String {
  guard let data = try? JSONEncoder().encode(value),
    let string = String(data: data, encoding: .utf8)
  else {
    return "{}"
  }

  return string
}

private func waitForValue<T>(_ operation: @escaping () async -> T) -> T {
  let semaphore = DispatchSemaphore(value: 0)
  var result: T!

  Task {
    result = await operation()
    semaphore.signal()
  }

  semaphore.wait()
  return result
}

private func decodeFloatSamples(from data: Data) throws -> [Float] {
  let stride = MemoryLayout<Float>.size
  guard data.count.isMultiple(of: stride) else {
    throw SoniqoBridgeError.message("Invalid audio chunk received by Soniqo.")
  }

  let count = data.count / stride
  var samples = [Float]()
  samples.reserveCapacity(count)

  data.withUnsafeBytes { bytes in
    for index in 0..<count {
      let bits = bytes.loadUnaligned(fromByteOffset: index * stride, as: UInt32.self)
      samples.append(Float(bitPattern: UInt32(littleEndian: bits)))
    }
  }

  return samples
}

private actor SoniqoBridge {
  static let shared = SoniqoBridge()

  private var loadedModels: [SpeechModelKind: LoadedSpeechModel] = [:]
  private var modelTasks: [SpeechModelKind: Task<LoadedSpeechModel, Error>] = [:]
  private var downloadStates: [SpeechModelKind: ModelDownloadPayload] = [:]
  private var activeStreamingSessions: [TranscriptSource: StreamingSession] = [:]

  func cacheDirectory(modelId: String) -> String {
    guard let kind = SpeechModelKind.resolve(modelId) else {
      return ""
    }

    refreshReadyState(for: kind)
    return kind.cacheDirectoryPath()
  }

  func modelDownloadStateJSON(modelId: String) -> String {
    guard let kind = SpeechModelKind.resolve(modelId) else {
      return encodeJSON(
        ModelDownloadPayload(
          status: "error",
          currentFile: nil,
          progressPercent: nil,
          localPath: "",
          error: "Unsupported Soniqo model."
        )
      )
    }

    refreshReadyState(for: kind)
    return encodeJSON(downloadState(for: kind))
  }

  func startModelDownload(modelId: String) {
    guard let kind = SpeechModelKind.resolve(modelId) else {
      return
    }

    refreshReadyState(for: kind)
    if kind.filesReady(), modelTasks[kind] == nil {
      var state = downloadState(for: kind)
      state.status = "ready"
      state.currentFile = nil
      state.error = nil
      downloadStates[kind] = state
      return
    }

    if modelTasks[kind] != nil {
      var state = downloadState(for: kind)
      state.status = "downloading"
      downloadStates[kind] = state
      return
    }

    var state = downloadState(for: kind)
    state.status = "downloading"
    state.currentFile = "Preparing \(kind.label)..."
    state.progressPercent = nil
    state.error = nil
    downloadStates[kind] = state

    let task = Task.detached(priority: .utility) {
      try await kind.load { fraction, status in
        Task {
          await SoniqoBridge.shared.updateDownloadProgress(
            kind: kind,
            fraction: fraction,
            status: status
          )
        }
      }
    }

    modelTasks[kind] = task

    Task.detached {
      do {
        let model = try await task.value
        await SoniqoBridge.shared.finishModelLoad(kind: kind, model: model)
      } catch {
        await SoniqoBridge.shared.finishModelLoad(kind: kind, error: error)
      }
    }
  }

  func resetModel(modelId: String) {
    guard let kind = SpeechModelKind.resolve(modelId) else {
      return
    }

    loadedModels[kind] = nil
    modelTasks[kind] = nil
    refreshReadyState(for: kind)

    var state = downloadState(for: kind)
    if state.status != "ready" {
      state.status = "idle"
    }
    state.currentFile = nil
    state.progressPercent = nil
    state.error = nil
    downloadStates[kind] = state
  }

  func startLiveJSON(modelId: String) async -> String {
    do {
      guard let kind = SpeechModelKind.resolve(modelId) else {
        throw SoniqoBridgeError.message("Unsupported Soniqo model: \(modelId)")
      }
      guard kind.isStreamingCapable else {
        throw SoniqoBridgeError.message("\(kind.label) does not support realtime transcription.")
      }

      let model = try await ensureModelLoaded(kind).asStreamingModel()
      activeStreamingSessions = [
        .microphone: try model.createSession(),
        .system: try model.createSession(),
      ]
      return encodeJSON(StatusPayload(running: true, error: nil))
    } catch {
      activeStreamingSessions = [:]
      return encodeJSON(StatusPayload(running: false, error: error.localizedDescription))
    }
  }

  func stopLiveJSON() -> String {
    activeStreamingSessions = [:]
    return encodeJSON(StatusPayload(running: false, error: nil))
  }

  func appendLiveJSON(source: String, samplesData: Data) -> String {
    do {
      guard let transcriptSource = TranscriptSource(rawValue: source) else {
        throw SoniqoBridgeError.message("Unsupported Soniqo transcript source: \(source)")
      }
      guard let session = activeStreamingSessions[transcriptSource] else {
        throw SoniqoBridgeError.message("No active Soniqo transcription session.")
      }

      let samples = try decodeFloatSamples(from: samplesData)
      let partials = try session.pushAudio(samples).map { partial in
        LivePartialPayload(
          source: transcriptSource.rawValue,
          text: partial.text,
          isFinal: partial.isFinal
        )
      }
      return encodeJSON(LiveAppendPayload(partials: partials, error: nil))
    } catch {
      return encodeJSON(LiveAppendPayload(partials: [], error: error.localizedDescription))
    }
  }

  func finalizeLiveJSON(source: String) -> String {
    do {
      guard let transcriptSource = TranscriptSource(rawValue: source) else {
        throw SoniqoBridgeError.message("Unsupported Soniqo transcript source: \(source)")
      }
      guard let session = activeStreamingSessions[transcriptSource] else {
        throw SoniqoBridgeError.message("No active Soniqo transcription session.")
      }

      let partials = try session.finalize().map { partial in
        LivePartialPayload(
          source: transcriptSource.rawValue,
          text: partial.text,
          isFinal: partial.isFinal
        )
      }
      return encodeJSON(LiveAppendPayload(partials: partials, error: nil))
    } catch {
      return encodeJSON(LiveAppendPayload(partials: [], error: error.localizedDescription))
    }
  }

  func transcribeAudioFileJSON(modelId: String, audioPath: String, language: String) async -> String
  {
    do {
      guard let kind = SpeechModelKind.resolve(modelId) else {
        throw SoniqoBridgeError.message("Unsupported Soniqo model: \(modelId)")
      }

      let trimmedLanguage = language.trimmingCharacters(in: .whitespacesAndNewlines)
      let url = URL(fileURLWithPath: audioPath)
      let audio = try AudioFileLoader.load(
        url: url,
        targetSampleRate: soniqoFileTranscriptionSampleRate
      )
      let model = try await ensureModelLoaded(kind)
      let text = try transcribeFileAudio(
        model: model,
        kind: kind,
        audio: audio,
        sampleRate: soniqoFileTranscriptionSampleRate,
        language: trimmedLanguage.isEmpty ? nil : trimmedLanguage
      )

      return encodeJSON(
        FileTranscriptionPayload(
          text: text,
          durationSeconds: Double(audio.count) / Double(soniqoFileTranscriptionSampleRate),
          error: nil
        )
      )
    } catch {
      return encodeJSON(
        FileTranscriptionPayload(
          text: "",
          durationSeconds: 0,
          error: error.localizedDescription
        )
      )
    }
  }

  private func transcribeFileAudio(
    model: LoadedSpeechModel,
    kind: SpeechModelKind,
    audio: [Float],
    sampleRate: Int,
    language: String?
  ) throws -> String {
    guard !audio.isEmpty else {
      return ""
    }

    guard let chunkSeconds = kind.fileTranscriptionChunkSeconds else {
      return try transcribeFileAudioChunk(
        model: model,
        kind: kind,
        audio: audio,
        sampleRate: sampleRate,
        language: language
      )
    }

    let chunkSampleCount = max(sampleRate, Int((Double(sampleRate) * chunkSeconds).rounded(.up)))
    let minimumTrailingSamples =
      kind.minimumFileTranscriptionChunkSeconds.map {
        max(sampleRate, Int((Double(sampleRate) * $0).rounded(.up)))
      } ?? 0
    let maximumChunkSamples =
      kind.maximumFileTranscriptionChunkSeconds.map {
        max(chunkSampleCount, Int((Double(sampleRate) * $0).rounded(.up)))
      } ?? chunkSampleCount
    let ranges = fileTranscriptionChunkRanges(
      sampleCount: audio.count,
      preferredChunkSamples: chunkSampleCount,
      minimumTrailingSamples: minimumTrailingSamples,
      maximumChunkSamples: maximumChunkSamples
    )

    guard ranges.count > 1 else {
      return try transcribeFileAudioChunk(
        model: model,
        kind: kind,
        audio: audio,
        sampleRate: sampleRate,
        language: language
      )
    }

    var chunks: [String] = []

    for range in ranges {
      let text = try autoreleasepool {
        try transcribeFileAudioChunk(
          model: model,
          kind: kind,
          audio: Array(audio[range]),
          sampleRate: sampleRate,
          language: language
        )
      }
      let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
      if !trimmed.isEmpty {
        chunks.append(trimmed)
      }
    }

    return chunks.joined(separator: " ")
  }

  private func transcribeFileAudioChunk(
    model: LoadedSpeechModel,
    kind: SpeechModelKind,
    audio: [Float],
    sampleRate: Int,
    language: String?
  ) throws -> String {
    let normalizedAudio = normalizedFileTranscriptionAudio(
      kind: kind,
      audio: audio,
      sampleRate: sampleRate
    )
    return try model.transcribe(audio: normalizedAudio, sampleRate: sampleRate, language: language)
  }

  private func normalizedFileTranscriptionAudio(
    kind: SpeechModelKind,
    audio: [Float],
    sampleRate: Int
  ) -> [Float] {
    guard kind == .parakeetBatch else {
      return audio
    }

    let minimumSamples = max(
      sampleRate,
      Int((Double(sampleRate) * parakeetBatchMinimumChunkSeconds).rounded(.up))
    )
    guard audio.count < minimumSamples else {
      return audio
    }

    var padded = audio
    padded.append(contentsOf: repeatElement(Float.zero, count: minimumSamples - audio.count))
    return padded
  }

  private func fileTranscriptionChunkRanges(
    sampleCount: Int,
    preferredChunkSamples: Int,
    minimumTrailingSamples: Int,
    maximumChunkSamples: Int
  ) -> [Range<Int>] {
    guard sampleCount > preferredChunkSamples else {
      return [0..<sampleCount]
    }

    var ranges: [Range<Int>] = []
    var start = 0

    while start < sampleCount {
      let end = min(sampleCount, start + preferredChunkSamples)
      ranges.append(start..<end)
      start = end
    }

    guard minimumTrailingSamples > 0, ranges.count >= 2, let trailingRange = ranges.last else {
      return ranges
    }

    let trailingSamples = trailingRange.upperBound - trailingRange.lowerBound
    guard trailingSamples < minimumTrailingSamples else {
      return ranges
    }

    let previousIndex = ranges.count - 2
    let previousRange = ranges[previousIndex]
    let mergedSamples = trailingRange.upperBound - previousRange.lowerBound
    guard mergedSamples <= maximumChunkSamples else {
      return ranges
    }

    ranges.removeLast()
    ranges[previousIndex] = previousRange.lowerBound..<trailingRange.upperBound
    return ranges
  }

  private func ensureModelLoaded(_ kind: SpeechModelKind) async throws -> LoadedSpeechModel {
    refreshReadyState(for: kind)

    if let model = loadedModels[kind] {
      return model
    }

    if let task = modelTasks[kind] {
      let loaded = try await task.value
      loadedModels[kind] = loaded
      return loaded
    }

    let loaded = try await kind.load(progressHandler: nil)
    loadedModels[kind] = loaded
    refreshReadyState(for: kind)
    return loaded
  }

  private func updateDownloadProgress(kind: SpeechModelKind, fraction: Double, status: String) {
    var state = downloadState(for: kind)
    state.status = "downloading"
    state.localPath = kind.cacheDirectoryPath()
    state.error = nil

    let percent = Int(max(0.0, min(1.0, fraction)) * 100.0)
    let statusText = status.trimmingCharacters(in: .whitespacesAndNewlines)
    state.progressPercent = percent
    state.currentFile = statusText.isEmpty ? "Preparing \(kind.label)..." : statusText
    downloadStates[kind] = state
  }

  private func finishModelLoad(kind: SpeechModelKind, model: LoadedSpeechModel) {
    loadedModels[kind] = model
    modelTasks[kind] = nil

    var state = downloadState(for: kind)
    state.localPath = kind.cacheDirectoryPath()
    state.status = "ready"
    state.currentFile = nil
    state.progressPercent = nil
    state.error = nil
    downloadStates[kind] = state
  }

  private func finishModelLoad(kind: SpeechModelKind, error: Error) {
    modelTasks[kind] = nil

    var state = downloadState(for: kind)
    state.localPath = kind.cacheDirectoryPath()
    state.status = "error"
    state.currentFile = nil
    state.progressPercent = nil
    state.error = error.localizedDescription
    downloadStates[kind] = state
  }

  private func refreshReadyState(for kind: SpeechModelKind) {
    var state = downloadState(for: kind)
    state.localPath = kind.cacheDirectoryPath()

    guard modelTasks[kind] == nil else {
      downloadStates[kind] = state
      return
    }

    if kind.filesReady() {
      state.status = "ready"
      state.error = nil
      state.currentFile = nil
      state.progressPercent = nil
    } else if state.status == "ready" {
      state.status = "idle"
      state.currentFile = nil
      state.progressPercent = nil
      state.error = nil
      loadedModels[kind] = nil
    } else if state.localPath.isEmpty {
      state.status = "idle"
    }

    downloadStates[kind] = state
  }

  private func downloadState(for kind: SpeechModelKind) -> ModelDownloadPayload {
    if let state = downloadStates[kind] {
      return state
    }

    return ModelDownloadPayload(
      status: "idle",
      currentFile: nil,
      progressPercent: nil,
      localPath: kind.cacheDirectoryPath(),
      error: nil
    )
  }
}

@_cdecl("_soniqo_model_cache_dir")
public func _soniqo_model_cache_dir(modelId: SRString) -> SRString {
  SRString(
    waitForValue {
      await SoniqoBridge.shared.cacheDirectory(modelId: modelId.toString())
    })
}

@_cdecl("_soniqo_model_download_state")
public func _soniqo_model_download_state(modelId: SRString) -> SRString {
  SRString(
    waitForValue {
      await SoniqoBridge.shared.modelDownloadStateJSON(modelId: modelId.toString())
    })
}

@_cdecl("_soniqo_model_start_download")
public func _soniqo_model_start_download(modelId: SRString) -> Bool {
  waitForValue {
    await SoniqoBridge.shared.startModelDownload(modelId: modelId.toString())
    return true
  }
}

@_cdecl("_soniqo_model_reset")
public func _soniqo_model_reset(modelId: SRString) -> Bool {
  waitForValue {
    await SoniqoBridge.shared.resetModel(modelId: modelId.toString())
    return true
  }
}

@_cdecl("_soniqo_transcribe_audio_file")
public func _soniqo_transcribe_audio_file(
  modelId: SRString,
  audioPath: SRString,
  language: SRString
) -> SRString {
  SRString(
    waitForValue {
      await SoniqoBridge.shared.transcribeAudioFileJSON(
        modelId: modelId.toString(),
        audioPath: audioPath.toString(),
        language: language.toString()
      )
    })
}

@_cdecl("_soniqo_live_start")
public func _soniqo_live_start(modelId: SRString) -> SRString {
  SRString(
    waitForValue {
      await SoniqoBridge.shared.startLiveJSON(modelId: modelId.toString())
    })
}

@_cdecl("_soniqo_live_append")
public func _soniqo_live_append(source: SRString, samples: SRData) -> SRString {
  SRString(
    waitForValue {
      await SoniqoBridge.shared.appendLiveJSON(
        source: source.toString(),
        samplesData: Data(samples.toArray())
      )
    })
}

@_cdecl("_soniqo_live_finalize")
public func _soniqo_live_finalize(source: SRString) -> SRString {
  SRString(
    waitForValue {
      await SoniqoBridge.shared.finalizeLiveJSON(source: source.toString())
    })
}

@_cdecl("_soniqo_live_stop")
public func _soniqo_live_stop() -> SRString {
  SRString(waitForValue { await SoniqoBridge.shared.stopLiveJSON() })
}
