import AVFoundation
import AudioCommon
import Foundation
import OmnilingualASR
import ParakeetASR
import ParakeetStreamingASR
import Qwen3ASR
import SpeechVAD
import SwiftRs

private enum SpeechBridgeError: LocalizedError {
  case message(String)

  var errorDescription: String? {
    switch self {
    case .message(let message):
      return message
    }
  }
}

private enum ProcessingMode: String {
  case realtime
  case batch
}

private enum SpeechModelKind: String, CaseIterable {
  case parakeetStreaming
  case parakeetBatch
  case omnilingual
  case qwen3Small
  case qwen3Large

  static func resolve(_ identifier: String) -> Self? {
    Self(rawValue: identifier) ?? Self.allCases.first(where: { $0.repo == identifier })
  }

  var label: String {
    switch self {
    case .parakeetStreaming:
      return "Parakeet Streaming"
    case .parakeetBatch:
      return "Parakeet Batch"
    case .omnilingual:
      return "Omnilingual"
    case .qwen3Small:
      return "Qwen3 0.6B"
    case .qwen3Large:
      return "Qwen3 1.7B"
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
      return .qwen3(
        try await Qwen3ASRModel.fromPretrained(
          modelId: repo,
          offlineMode: offlineMode,
          progressHandler: progressHandler
        )
      )
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
  case qwen3(Qwen3ASRModel)

  func asStreamingModel() throws -> ParakeetStreamingASRModel {
    guard case .streaming(let model) = self else {
      throw SpeechBridgeError.message("The selected model does not support realtime transcription.")
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
    case .qwen3(let model):
      return model.transcribe(audio: audio, sampleRate: sampleRate, language: languageHint)
    }
  }
}

private struct ModelDownloadPayload: Codable {
  var status: String
  var currentFile: String?
  var progressPercent: Int?
  var localPath: String
  var error: String?
}

private enum TranscriptSource: String, CaseIterable, Codable {
  case microphone
  case system
  case mixed
}

private struct TranscriptEntryPayload: Codable {
  var source: String
  var text: String
}

private struct TranscriptionPayload: Codable {
  var running: Bool
  var text: String
  var error: String?
  var entries: [TranscriptEntryPayload]
  var audioPath: String
  var mode: String?

  static let empty = TranscriptionPayload(
    running: false,
    text: "",
    error: nil,
    entries: [],
    audioPath: "",
    mode: nil
  )
}

private struct FileTranscriptionPayload: Codable {
  var text: String
  var error: String?
}

private struct DiarizationSegmentPayload: Codable, Sendable {
  var speaker: String
  var startSeconds: Double
  var endSeconds: Double
}

private struct FileDiarizationPayload: Codable, Sendable {
  var segments: [DiarizationSegmentPayload]
  var speakerCount: Int
  var pipelineSource: String
  var error: String?
}

private struct SpeakerEmbeddingRequestPayload: Codable, Sendable {
  var speaker: String
  var segments: [DiarizationSegmentPayload]
}

private struct SpeakerEmbeddingSamplePayload: Codable, Sendable {
  var startSeconds: Double
  var endSeconds: Double
  var durationSeconds: Double
  var embedding: [Float]
}

private struct SpeakerEmbeddingPayload: Codable, Sendable {
  var speaker: String
  var embedding: [Float]
  var samples: [SpeakerEmbeddingSamplePayload]
}

private struct FileSpeakerEmbeddingPayload: Codable, Sendable {
  var speakers: [SpeakerEmbeddingPayload]
  var error: String?
}

private let diarizationPipelineSource = "speech-swift / sortformer"

private func diarizationSegmentDuration(_ segment: DiarizationSegmentPayload) -> Double {
  max(0, segment.endSeconds - segment.startSeconds)
}

private func trimmedSpeakerEmbeddingSegment(
  _ segment: DiarizationSegmentPayload,
  minimumDuration: Double,
  maximumDuration: Double
) -> DiarizationSegmentPayload? {
  let start = max(0, segment.startSeconds)
  let end = max(start, segment.endSeconds)
  let duration = end - start
  guard duration >= minimumDuration else {
    return nil
  }

  if duration <= maximumDuration {
    return DiarizationSegmentPayload(speaker: segment.speaker, startSeconds: start, endSeconds: end)
  }

  let midpoint = start + duration / 2
  let clippedStart = max(0, midpoint - maximumDuration / 2)
  return DiarizationSegmentPayload(
    speaker: segment.speaker,
    startSeconds: clippedStart,
    endSeconds: clippedStart + maximumDuration
  )
}

private func selectSpeakerEmbeddingSegments(
  _ segments: [DiarizationSegmentPayload],
  limit: Int
) -> [DiarizationSegmentPayload] {
  let primary = segments.compactMap {
    trimmedSpeakerEmbeddingSegment($0, minimumDuration: 2.5, maximumDuration: 6.0)
  }
  let fallback = segments.compactMap {
    trimmedSpeakerEmbeddingSegment($0, minimumDuration: 1.5, maximumDuration: 4.0)
  }
  let candidates = primary.isEmpty ? fallback : primary

  return Array(
    candidates
      .sorted { lhs, rhs in
        let lhsDuration = diarizationSegmentDuration(lhs)
        let rhsDuration = diarizationSegmentDuration(rhs)
        if lhsDuration == rhsDuration {
          return lhs.startSeconds < rhs.startSeconds
        }

        return lhsDuration > rhsDuration
      }
      .prefix(max(1, limit))
  )
}

private func sliceAudio(
  _ audio: [Float],
  sampleRate: Int,
  startSeconds: Double,
  endSeconds: Double
) -> [Float] {
  guard !audio.isEmpty, sampleRate > 0 else {
    return []
  }

  let clampedStart = max(0, startSeconds)
  let clampedEnd = max(clampedStart, endSeconds)
  let startIndex = min(audio.count, max(0, Int(floor(clampedStart * Double(sampleRate)))))
  let endIndex = min(audio.count, max(startIndex, Int(ceil(clampedEnd * Double(sampleRate)))))
  guard endIndex > startIndex else {
    return []
  }

  return Array(audio[startIndex..<endIndex])
}

private func normalizedEmbeddingCentroid(_ embeddings: [[Float]]) -> [Float] {
  guard let first = embeddings.first, !first.isEmpty else {
    return []
  }

  var centroid = [Float](repeating: 0, count: first.count)
  for embedding in embeddings where embedding.count == centroid.count {
    for (index, value) in embedding.enumerated() {
      centroid[index] += value
    }
  }

  let norm = sqrt(
    centroid.reduce(Float.zero) { partialResult, value in
      partialResult + (value * value)
    })
  guard norm > 0 else {
    return centroid
  }

  return centroid.map { $0 / norm }
}

private func constrainDiarizedSegments(
  _ segments: [DiarizedSegment],
  requestedSpeakerCount: Int?
) -> [DiarizedSegment] {
  guard
    let requestedSpeakerCount,
    requestedSpeakerCount > 0,
    !segments.isEmpty
  else {
    return segments
  }

  if requestedSpeakerCount == 1 {
    return segments.map { segment in
      DiarizedSegment(
        startTime: segment.startTime,
        endTime: segment.endTime,
        speakerId: 0
      )
    }
  }

  let speakerDurations = Dictionary(grouping: segments, by: \.speakerId)
    .mapValues { speakerSegments in
      speakerSegments.reduce(Float.zero) { partialResult, segment in
        partialResult + segment.duration
      }
    }

  if speakerDurations.count <= requestedSpeakerCount {
    return compactDiarizedSpeakerIds(segments)
  }

  let retainedSpeakerIds = Set(
    speakerDurations
      .sorted { lhs, rhs in
        if lhs.value == rhs.value {
          return lhs.key < rhs.key
        }

        return lhs.value > rhs.value
      }
      .prefix(requestedSpeakerCount)
      .map(\.key)
  )

  let retainedSegments = segments.filter { retainedSpeakerIds.contains($0.speakerId) }
  let fallbackSpeakerId = retainedSpeakerIds.min() ?? 0
  let remapped = segments.map { segment in
    let speakerId: Int
    if retainedSpeakerIds.contains(segment.speakerId) {
      speakerId = segment.speakerId
    } else {
      speakerId =
        retainedSegments.min(by: { lhs, rhs in
          diarizedSegmentDistance(from: segment, to: lhs)
            < diarizedSegmentDistance(from: segment, to: rhs)
        })?.speakerId ?? fallbackSpeakerId
    }

    return DiarizedSegment(
      startTime: segment.startTime,
      endTime: segment.endTime,
      speakerId: speakerId
    )
  }

  return compactDiarizedSpeakerIds(remapped)
}

private func diarizedSegmentDistance(from lhs: DiarizedSegment, to rhs: DiarizedSegment) -> Float {
  if lhs.endTime >= rhs.startTime && rhs.endTime >= lhs.startTime {
    return 0
  }

  return min(abs(lhs.startTime - rhs.endTime), abs(rhs.startTime - lhs.endTime))
}

private func compactDiarizedSpeakerIds(_ segments: [DiarizedSegment]) -> [DiarizedSegment] {
  let speakerIds = Array(Set(segments.map(\.speakerId))).sorted()
  let speakerMap = Dictionary(uniqueKeysWithValues: speakerIds.enumerated().map { ($1, $0) })

  return segments.map { segment in
    DiarizedSegment(
      startTime: segment.startTime,
      endTime: segment.endTime,
      speakerId: speakerMap[segment.speakerId] ?? segment.speakerId
    )
  }
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
    throw SpeechBridgeError.message("Invalid audio chunk received from native capture.")
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

private final class WAVCaptureWriter {
  private let url: URL
  private let sampleRate: Int
  private let queue = DispatchQueue(
    label: "com.johnjeong.unsigned.speech-swift.capture",
    qos: .utility
  )

  private var fileHandle: FileHandle?
  private var sampleCount = 0
  private var closed = false
  private var lastErrorMessage: String?

  init(url: URL, sampleRate: Int = 16000) throws {
    self.url = url
    self.sampleRate = sampleRate

    let fileManager = FileManager.default
    try fileManager.createDirectory(
      at: url.deletingLastPathComponent(),
      withIntermediateDirectories: true,
      attributes: nil
    )

    if fileManager.fileExists(atPath: url.path) {
      do {
        sampleCount = try Self.existingSampleCount(at: url)
      } catch {
        try? fileManager.removeItem(at: url)
        sampleCount = 0
      }
    }

    if !fileManager.fileExists(atPath: url.path) {
      fileManager.createFile(
        atPath: url.path, contents: Self.headerData(sampleRate: sampleRate, sampleCount: 0))
    }

    let handle = try FileHandle(forWritingTo: url)
    try handle.seekToEnd()
    fileHandle = handle
  }

  func append(_ samples: [Float]) {
    guard !samples.isEmpty else {
      return
    }

    let pcmData = Self.pcmData(from: samples)
    queue.async { [self] in
      guard !closed, let fileHandle else {
        return
      }

      if lastErrorMessage != nil {
        return
      }

      do {
        try fileHandle.seekToEnd()
        try fileHandle.write(contentsOf: pcmData)
        sampleCount += samples.count
      } catch {
        lastErrorMessage = "Failed to write meeting audio: \(error.localizedDescription)"
      }
    }
  }

  func finish() throws -> String {
    try queue.sync { [self] in
      if closed {
        return
      }

      if let lastErrorMessage {
        throw SpeechBridgeError.message(lastErrorMessage)
      }

      closed = true

      guard let fileHandle else {
        throw SpeechBridgeError.message("Meeting audio writer is unavailable.")
      }

      do {
        try fileHandle.seek(toOffset: 0)
        try fileHandle.write(
          contentsOf: Self.headerData(sampleRate: sampleRate, sampleCount: sampleCount))
        try fileHandle.close()
        self.fileHandle = nil
      } catch {
        self.fileHandle = nil
        throw SpeechBridgeError.message(
          "Failed to finalize meeting audio: \(error.localizedDescription)")
      }
    }

    return url.path
  }

  func cancel(removeFile: Bool) {
    queue.sync { [self] in
      if !closed {
        closed = true
        try? fileHandle?.close()
        fileHandle = nil
      }
    }

    if removeFile {
      try? FileManager.default.removeItem(at: url)
    }
  }

  private static func existingSampleCount(at url: URL) throws -> Int {
    let handle = try FileHandle(forReadingFrom: url)
    defer { try? handle.close() }

    let header = try handle.read(upToCount: 44) ?? Data()
    guard header.count >= 44,
      String(data: header[0..<4], encoding: .ascii) == "RIFF",
      String(data: header[8..<12], encoding: .ascii) == "WAVE",
      String(data: header[36..<40], encoding: .ascii) == "data"
    else {
      throw SpeechBridgeError.message("Invalid WAV file at \(url.path)")
    }

    let dataSize = Int(header[40..<44].withUnsafeBytes { $0.loadUnaligned(as: UInt32.self) })
    return dataSize / 2
  }

  private static func headerData(sampleRate: Int, sampleCount: Int) -> Data {
    let numChannels: UInt16 = 1
    let bitsPerSample: UInt16 = 16
    let bytesPerSample = Int(bitsPerSample) / 8
    let dataSize = sampleCount * bytesPerSample
    let fileSize = 36 + dataSize

    var data = Data(capacity: fileSize + 8)
    data.append(contentsOf: "RIFF".utf8)
    appendUInt32(&data, UInt32(fileSize))
    data.append(contentsOf: "WAVE".utf8)
    data.append(contentsOf: "fmt ".utf8)
    appendUInt32(&data, 16)
    appendUInt16(&data, 1)
    appendUInt16(&data, numChannels)
    appendUInt32(&data, UInt32(sampleRate))
    appendUInt32(&data, UInt32(sampleRate * Int(numChannels) * bytesPerSample))
    appendUInt16(&data, numChannels * UInt16(bytesPerSample))
    appendUInt16(&data, bitsPerSample)
    data.append(contentsOf: "data".utf8)
    appendUInt32(&data, UInt32(dataSize))
    return data
  }

  private static func pcmData(from samples: [Float]) -> Data {
    var data = Data(capacity: samples.count * 2)
    for sample in samples {
      let clamped = max(-1.0, min(1.0, sample))
      var int16Value = Int16(clamped * 32767.0).littleEndian
      data.append(Data(bytes: &int16Value, count: 2))
    }
    return data
  }

  private static func appendUInt32(_ data: inout Data, _ value: UInt32) {
    var v = value.littleEndian
    data.append(Data(bytes: &v, count: 4))
  }

  private static func appendUInt16(_ data: inout Data, _ value: UInt16) {
    var v = value.littleEndian
    data.append(Data(bytes: &v, count: 2))
  }
}

private final class LiveTranscriptionSession {
  private let captureWriter: WAVCaptureWriter
  private let finalRecordingURL: URL
  private let workingRecordingURL: URL
  private let recordingPath: String
  private let mode: ProcessingMode
  private let streamingSessions: [TranscriptSource: StreamingSession]
  private let stateLock = NSLock()
  private let bufferLock = NSLock()
  private let processingQueue = DispatchQueue(
    label: "com.johnjeong.unsigned.speech-swift.processing",
    qos: .userInitiated
  )

  private var processingTimer: DispatchSourceTimer?
  private var bufferedSamplesBySource: [TranscriptSource: [Float]] = [:]
  private var finalizedEntries: [TranscriptEntryPayload] = []
  private var partialTexts: [TranscriptSource: String] = [:]
  private var running = false
  private var acceptingInput = false
  private var errorMessage: String?

  init(
    mode: ProcessingMode,
    recordingURL: URL,
    streamingModel: ParakeetStreamingASRModel? = nil
  ) throws {
    self.mode = mode
    finalRecordingURL = recordingURL
    workingRecordingURL = Self.workingRecordingURL(for: recordingURL)
    recordingPath = recordingURL.path
    try Self.prepareWorkingRecording(at: workingRecordingURL, from: finalRecordingURL)
    captureWriter = try WAVCaptureWriter(url: workingRecordingURL)
    if let streamingModel {
      streamingSessions = [
        .microphone: try streamingModel.createSession(),
        .system: try streamingModel.createSession(),
      ]
    } else {
      streamingSessions = [:]
    }
  }

  func start() throws {
    if !streamingSessions.isEmpty {
      let timer = DispatchSource.makeTimerSource(queue: processingQueue)
      timer.schedule(deadline: .now(), repeating: .milliseconds(250))
      timer.setEventHandler { [weak self] in
        self?.processBufferedSamples()
      }
      timer.resume()
      processingTimer = timer
    }

    stateLock.lock()
    running = true
    acceptingInput = true
    errorMessage = nil
    stateLock.unlock()
  }

  func snapshot() -> TranscriptionPayload {
    stateLock.lock()
    defer { stateLock.unlock() }

    let entries = snapshotEntriesLocked()

    return TranscriptionPayload(
      running: running,
      text: transcriptText(from: entries),
      error: errorMessage,
      entries: entries,
      audioPath: recordingPath,
      mode: mode.rawValue
    )
  }

  func stop() throws -> TranscriptionPayload {
    stateLock.lock()
    let wasRunning = running
    running = false
    acceptingInput = false
    stateLock.unlock()

    let timer = processingTimer
    processingTimer = nil
    timer?.cancel()

    if wasRunning && !streamingSessions.isEmpty {
      processingQueue.sync {
        processBufferedSamples()
        finalizeStreamingSession()
      }
    }

    _ = try captureWriter.finish()
    try Self.encodeWorkingRecording(from: workingRecordingURL, to: finalRecordingURL)
    try? FileManager.default.removeItem(at: workingRecordingURL)
    let snapshot = snapshot()
    return TranscriptionPayload(
      running: false,
      text: snapshot.text,
      error: snapshot.error,
      entries: snapshot.entries,
      audioPath: finalRecordingURL.path,
      mode: mode.rawValue
    )
  }

  func cancel(removeRecording: Bool) {
    stateLock.lock()
    running = false
    acceptingInput = false
    stateLock.unlock()
    let timer = processingTimer
    processingTimer = nil
    timer?.cancel()
    captureWriter.cancel(removeFile: removeRecording)
    if removeRecording {
      try? FileManager.default.removeItem(at: finalRecordingURL)
    }
  }

  func requestStop() {
    stateLock.lock()
    acceptingInput = false
    stateLock.unlock()
  }

  func ingest(mixedSamples: [Float], microphoneSamples: [Float], systemSamples: [Float]) {
    stateLock.lock()
    let isRunning = running
    let isAcceptingInput = acceptingInput
    stateLock.unlock()

    guard isRunning && isAcceptingInput else {
      return
    }

    append(
      mixedSamples: mixedSamples,
      sourceSamples: [
        .microphone: microphoneSamples,
        .system: systemSamples,
      ]
    )
  }

  private func append(
    mixedSamples: [Float],
    sourceSamples: [TranscriptSource: [Float]]
  ) {
    guard !mixedSamples.isEmpty else {
      return
    }

    captureWriter.append(mixedSamples)

    guard !streamingSessions.isEmpty else {
      return
    }

    bufferLock.lock()
    for source in TranscriptSource.allCases where source != .mixed {
      guard let samples = sourceSamples[source], !samples.isEmpty else {
        continue
      }

      bufferedSamplesBySource[source, default: []].append(contentsOf: samples)
    }
    bufferLock.unlock()
  }

  private func takeBufferedSamples(for source: TranscriptSource) -> [Float] {
    bufferLock.lock()
    defer { bufferLock.unlock() }

    let samples = bufferedSamplesBySource[source] ?? []
    bufferedSamplesBySource[source] = []
    return samples
  }

  private func processBufferedSamples() {
    for source in TranscriptSource.allCases where source != .mixed {
      guard let streamingSession = streamingSessions[source] else {
        continue
      }

      let samples = takeBufferedSamples(for: source)
      guard !samples.isEmpty else {
        continue
      }

      do {
        apply(try streamingSession.pushAudio(samples), source: source)
      } catch {
        fail(
          "speech-swift failed while processing \(source.rawValue) audio: \(error.localizedDescription)"
        )
        return
      }
    }
  }

  private func finalizeStreamingSession() {
    for source in TranscriptSource.allCases where source != .mixed {
      guard let streamingSession = streamingSessions[source] else {
        continue
      }

      do {
        apply(try streamingSession.finalize(), source: source)

        stateLock.lock()
        partialTexts[source] = nil
        stateLock.unlock()
      } catch {
        fail(
          "speech-swift failed while finalizing \(source.rawValue) audio: \(error.localizedDescription)"
        )
        return
      }
    }
  }

  private func apply(
    _ partials: [ParakeetStreamingASRModel.PartialTranscript],
    source: TranscriptSource
  ) {
    guard !partials.isEmpty else {
      return
    }

    stateLock.lock()
    defer { stateLock.unlock() }

    for partial in partials {
      let text = partial.text.trimmingCharacters(in: .whitespacesAndNewlines)

      if partial.isFinal {
        if !text.isEmpty {
          finalizedEntries.append(
            TranscriptEntryPayload(source: source.rawValue, text: text)
          )
        }
        partialTexts[source] = nil
        continue
      }

      if text.isEmpty {
        partialTexts[source] = nil
      } else {
        partialTexts[source] = text
      }
    }
  }

  private func fail(_ message: String) {
    stateLock.lock()
    errorMessage = message
    running = false
    acceptingInput = false
    stateLock.unlock()

    let timer = processingTimer
    processingTimer = nil
    timer?.cancel()
  }

  private func snapshotEntriesLocked() -> [TranscriptEntryPayload] {
    var entries = finalizedEntries

    for source in TranscriptSource.allCases where source != .mixed {
      let text = partialTexts[source]?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
      if !text.isEmpty {
        entries.append(TranscriptEntryPayload(source: source.rawValue, text: text))
      }
    }

    return entries
  }

  private func transcriptText(from entries: [TranscriptEntryPayload]) -> String {
    entries
      .map(\.text)
      .joined(separator: "\n")
  }

  private static func workingRecordingURL(for finalRecordingURL: URL) -> URL {
    finalRecordingURL
      .deletingPathExtension()
      .appendingPathExtension("recording.wav")
  }

  private static func legacyRecordingURL(for finalRecordingURL: URL) -> URL {
    finalRecordingURL
      .deletingPathExtension()
      .appendingPathExtension("wav")
  }

  private static func prepareWorkingRecording(
    at workingRecordingURL: URL, from finalRecordingURL: URL
  )
    throws
  {
    let fileManager = FileManager.default
    try fileManager.createDirectory(
      at: finalRecordingURL.deletingLastPathComponent(),
      withIntermediateDirectories: true,
      attributes: nil
    )

    if fileManager.fileExists(atPath: workingRecordingURL.path) {
      return
    }

    if fileManager.fileExists(atPath: finalRecordingURL.path) {
      try decodeSavedRecording(from: finalRecordingURL, to: workingRecordingURL)
      return
    }

    let legacyRecordingURL = legacyRecordingURL(for: finalRecordingURL)
    guard fileManager.fileExists(atPath: legacyRecordingURL.path) else {
      return
    }

    try? fileManager.removeItem(at: workingRecordingURL)
    try fileManager.moveItem(at: legacyRecordingURL, to: workingRecordingURL)
  }

  private static func encodeWorkingRecording(from sourceURL: URL, to destinationURL: URL) throws {
    let fileManager = FileManager.default
    let temporaryDestinationURL = destinationURL.appendingPathExtension("tmp")

    try runAudioConvert(
      arguments: [
        "-f", "m4af", "-d", "aac ", "-s", "1", sourceURL.path, temporaryDestinationURL.path,
      ],
      failureMessage: "Failed to convert meeting audio to M4A"
    )

    if fileManager.fileExists(atPath: destinationURL.path) {
      _ = try fileManager.replaceItemAt(destinationURL, withItemAt: temporaryDestinationURL)
      return
    }

    try fileManager.moveItem(at: temporaryDestinationURL, to: destinationURL)
  }

  private static func decodeSavedRecording(from sourceURL: URL, to destinationURL: URL) throws {
    try runAudioConvert(
      arguments: [
        "-f", "WAVE",
        "-d", "LEI16@16000",
        "-c", "1",
        sourceURL.path,
        destinationURL.path,
      ],
      failureMessage: "Failed to prepare the meeting audio for recording"
    )
  }

  private static func runAudioConvert(arguments: [String], failureMessage: String) throws {
    let fileManager = FileManager.default
    guard let destinationPath = arguments.last else {
      throw SpeechBridgeError.message("\(failureMessage).")
    }

    try fileManager.createDirectory(
      at: URL(fileURLWithPath: destinationPath).deletingLastPathComponent(),
      withIntermediateDirectories: true,
      attributes: nil
    )

    if fileManager.fileExists(atPath: destinationPath) {
      try? fileManager.removeItem(atPath: destinationPath)
    }

    let process = Process()
    process.executableURL = URL(fileURLWithPath: "/usr/bin/afconvert")
    process.arguments = arguments

    let outputPipe = Pipe()
    process.standardOutput = outputPipe
    process.standardError = outputPipe

    try process.run()
    process.waitUntilExit()

    guard process.terminationStatus == 0 else {
      let output = String(
        data: outputPipe.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8)?
        .trimmingCharacters(in: .whitespacesAndNewlines)
      let detail = (output?.isEmpty == false) ? ": \(output!)" : "."
      throw SpeechBridgeError.message("\(failureMessage)\(detail)")
    }
  }
}

private actor DiarizationPipeline {
  private var diarizer: SortformerDiarizer?
  private var diarizerTask: Task<SortformerDiarizer, Error>?

  func diarizeAudioFile(
    atPath path: String,
    speakerCount: Int
  ) async throws -> FileDiarizationPayload {
    let requestedSpeakerCount = speakerCount > 0 ? speakerCount : nil
    let url = URL(fileURLWithPath: path)
    let audio = try AudioFileLoader.load(url: url, targetSampleRate: 16000)
    let diarizer = try await ensureLoaded()
    let result = diarizer.diarize(audio: audio, sampleRate: 16000, config: .default)
    let segments = constrainDiarizedSegments(
      result.segments,
      requestedSpeakerCount: requestedSpeakerCount
    )

    return FileDiarizationPayload(
      segments: segments.map { segment in
        DiarizationSegmentPayload(
          speaker: "speaker_\(segment.speakerId)",
          startSeconds: Double(segment.startTime),
          endSeconds: Double(segment.endTime)
        )
      },
      speakerCount: Set(segments.map(\.speakerId)).count,
      pipelineSource: diarizationPipelineSource,
      error: nil
    )
  }

  private func ensureLoaded() async throws -> SortformerDiarizer {
    if let diarizer {
      return diarizer
    }

    if let task = diarizerTask {
      let diarizer = try await task.value
      self.diarizer = diarizer
      return diarizer
    }

    let task = Task<SortformerDiarizer, Error> {
      try await SortformerDiarizer.fromPretrained()
    }
    diarizerTask = task

    do {
      let diarizer = try await task.value
      self.diarizer = diarizer
      diarizerTask = nil
      return diarizer
    } catch {
      diarizerTask = nil
      throw error
    }
  }
}

private actor SpeakerEmbeddingPipeline {
  private var model: WeSpeakerModel?
  private var modelTask: Task<WeSpeakerModel, Error>?

  func analyzeAudioFile(
    atPath path: String,
    requests: [SpeakerEmbeddingRequestPayload],
    sampleLimit: Int
  ) async throws -> FileSpeakerEmbeddingPayload {
    let url = URL(fileURLWithPath: path)
    let audio = try AudioFileLoader.load(url: url, targetSampleRate: 16000)
    let model = try await ensureLoaded()

    let speakers = requests.map { request in
      let samples = selectSpeakerEmbeddingSegments(request.segments, limit: sampleLimit)
        .compactMap { segment -> SpeakerEmbeddingSamplePayload? in
          let clippedAudio = sliceAudio(
            audio,
            sampleRate: 16000,
            startSeconds: segment.startSeconds,
            endSeconds: segment.endSeconds
          )
          guard clippedAudio.count >= 16000 else {
            return nil
          }

          let embedding = model.embed(audio: clippedAudio, sampleRate: 16000)
          guard embedding.contains(where: { $0 != 0 }) else {
            return nil
          }

          return SpeakerEmbeddingSamplePayload(
            startSeconds: segment.startSeconds,
            endSeconds: segment.endSeconds,
            durationSeconds: diarizationSegmentDuration(segment),
            embedding: embedding
          )
        }

      return SpeakerEmbeddingPayload(
        speaker: request.speaker,
        embedding: normalizedEmbeddingCentroid(samples.map(\.embedding)),
        samples: samples
      )
    }

    return FileSpeakerEmbeddingPayload(speakers: speakers, error: nil)
  }

  private func ensureLoaded() async throws -> WeSpeakerModel {
    if let model {
      return model
    }

    if let task = modelTask {
      let model = try await task.value
      self.model = model
      return model
    }

    let task = Task<WeSpeakerModel, Error> {
      try await WeSpeakerModel.fromPretrained(engine: .coreml)
    }
    modelTask = task

    do {
      let model = try await task.value
      self.model = model
      modelTask = nil
      return model
    } catch {
      modelTask = nil
      throw error
    }
  }
}

private actor SpeechBridge {
  static let shared = SpeechBridge()

  private var loadedModels: [SpeechModelKind: LoadedSpeechModel] = [:]
  private var modelTasks: [SpeechModelKind: Task<LoadedSpeechModel, Error>] = [:]
  private var downloadStates: [SpeechModelKind: ModelDownloadPayload] = [:]
  private let diarizationPipeline = DiarizationPipeline()
  private let speakerEmbeddingPipeline = SpeakerEmbeddingPipeline()

  private var activeSession: LiveTranscriptionSession?
  private var activeMode: ProcessingMode?
  private var activeModelKind: SpeechModelKind?
  private var activeLanguage: String?

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
          error: "Unsupported speech model."
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
          await SpeechBridge.shared.updateDownloadProgress(
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
        await SpeechBridge.shared.finishModelLoad(kind: kind, model: model)
      } catch {
        await SpeechBridge.shared.finishModelLoad(kind: kind, error: error)
      }
    }
  }

  func resetModel(modelId: String) {
    guard let kind = SpeechModelKind.resolve(modelId) else {
      return
    }

    if activeModelKind == kind {
      activeSession?.cancel(removeRecording: false)
      activeSession = nil
      activeMode = nil
      activeModelKind = nil
      activeLanguage = nil
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

  func startTranscriptionJSON(
    mode: String,
    modelId: String,
    recordingPath: String,
    language: String
  ) async -> String {
    activeSession?.cancel(removeRecording: false)
    activeSession = nil
    activeMode = nil
    activeModelKind = nil
    activeLanguage = nil

    do {
      guard let processingMode = ProcessingMode(rawValue: mode) else {
        throw SpeechBridgeError.message("Unsupported transcription mode: \(mode)")
      }
      guard let kind = SpeechModelKind.resolve(modelId) else {
        throw SpeechBridgeError.message("Unsupported speech model: \(modelId)")
      }

      let recordingURL = URL(fileURLWithPath: recordingPath)
      let session: LiveTranscriptionSession
      switch processingMode {
      case .realtime:
        guard kind.isStreamingCapable else {
          throw SpeechBridgeError.message("\(kind.label) does not support realtime transcription.")
        }
        let model = try await ensureModelLoaded(kind).asStreamingModel()
        session = try LiveTranscriptionSession(
          mode: processingMode,
          recordingURL: recordingURL,
          streamingModel: model
        )
      case .batch:
        session = try LiveTranscriptionSession(
          mode: processingMode,
          recordingURL: recordingURL
        )
      }

      try session.start()
      activeSession = session
      activeMode = processingMode
      activeModelKind = kind
      let trimmedLanguage = language.trimmingCharacters(in: .whitespacesAndNewlines)
      activeLanguage = trimmedLanguage.isEmpty ? nil : trimmedLanguage
      return encodeJSON(session.snapshot())
    } catch {
      return encodeJSON(
        TranscriptionPayload(
          running: false,
          text: "",
          error: error.localizedDescription,
          entries: [],
          audioPath: recordingPath,
          mode: mode
        )
      )
    }
  }

  func transcribeAudioFileJSON(modelId: String, audioPath: String, language: String) async -> String
  {
    do {
      guard let kind = SpeechModelKind.resolve(modelId) else {
        throw SpeechBridgeError.message("Unsupported speech model: \(modelId)")
      }

      let trimmedLanguage = language.trimmingCharacters(in: .whitespacesAndNewlines)
      let text = try await transcribeRecordedAudio(
        atPath: audioPath,
        with: kind,
        language: trimmedLanguage.isEmpty ? nil : trimmedLanguage
      )

      return encodeJSON(
        FileTranscriptionPayload(
          text: text,
          error: nil
        )
      )
    } catch {
      return encodeJSON(
        FileTranscriptionPayload(
          text: "",
          error: error.localizedDescription
        )
      )
    }
  }

  func transcriptionStateJSON() async -> String {
    guard let activeSession else {
      return encodeJSON(TranscriptionPayload.empty)
    }

    let snapshot = activeSession.snapshot()
    guard !snapshot.running else {
      return encodeJSON(snapshot)
    }

    do {
      let finalSnapshot = try activeSession.stop()
      clearActiveSession()
      return encodeJSON(finalSnapshot)
    } catch {
      clearActiveSession()
      return encodeJSON(
        TranscriptionPayload(
          running: false,
          text: snapshot.text,
          error: error.localizedDescription,
          entries: snapshot.entries,
          audioPath: snapshot.audioPath,
          mode: snapshot.mode
        )
      )
    }
  }

  func stopTranscriptionJSON() async -> String {
    guard let activeSession else {
      return encodeJSON(TranscriptionPayload.empty)
    }

    let mode = activeMode
    let kind = activeModelKind
    let language = activeLanguage

    do {
      let snapshot = try activeSession.stop()
      clearActiveSession()

      guard mode == .batch, let kind else {
        return encodeJSON(snapshot)
      }

      let text = try await transcribeRecordedAudio(
        atPath: snapshot.audioPath,
        with: kind,
        language: language
      )
      return encodeJSON(
        TranscriptionPayload(
          running: false,
          text: text,
          error: snapshot.error,
          entries: text.isEmpty
            ? []
            : [TranscriptEntryPayload(source: TranscriptSource.mixed.rawValue, text: text)],
          audioPath: snapshot.audioPath,
          mode: mode?.rawValue
        )
      )
    } catch {
      let fallback = activeSession.snapshot()
      clearActiveSession()
      return encodeJSON(
        TranscriptionPayload(
          running: false,
          text: fallback.text,
          error: error.localizedDescription,
          entries: fallback.entries,
          audioPath: fallback.audioPath,
          mode: fallback.mode
        )
      )
    }
  }

  func requestStopTranscriptionJSON() async -> String {
    guard let activeSession else {
      return encodeJSON(TranscriptionPayload.empty)
    }

    activeSession.requestStop()
    return encodeJSON(activeSession.snapshot())
  }

  func appendTranscriptionAudio(
    mixedSamplesData: Data,
    microphoneSamplesData: Data,
    systemSamplesData: Data
  ) -> String {
    guard let activeSession else {
      return "No active transcription session."
    }

    do {
      let mixedSamples = try decodeFloatSamples(from: mixedSamplesData)
      let microphoneSamples = try decodeFloatSamples(from: microphoneSamplesData)
      let systemSamples = try decodeFloatSamples(from: systemSamplesData)
      activeSession.ingest(
        mixedSamples: mixedSamples,
        microphoneSamples: microphoneSamples,
        systemSamples: systemSamples
      )
      return ""
    } catch {
      return error.localizedDescription
    }
  }

  private func clearActiveSession() {
    activeSession = nil
    activeMode = nil
    activeModelKind = nil
    activeLanguage = nil
  }

  private func transcribeRecordedAudio(
    atPath path: String,
    with kind: SpeechModelKind,
    language: String?
  ) async throws -> String {
    let url = URL(fileURLWithPath: path)
    let audio = try AudioFileLoader.load(url: url, targetSampleRate: 16000)
    let model = try await ensureModelLoaded(kind)
    return try model.transcribe(audio: audio, sampleRate: 16000, language: language)
  }

  func diarizeAudioFileJSON(audioPath: String, speakerCount: Int) async -> String {
    do {
      let payload = try await diarizationPipeline.diarizeAudioFile(
        atPath: audioPath,
        speakerCount: speakerCount
      )
      return encodeJSON(payload)
    } catch {
      return encodeJSON(
        FileDiarizationPayload(
          segments: [],
          speakerCount: 0,
          pipelineSource: diarizationPipelineSource,
          error: error.localizedDescription
        )
      )
    }
  }

  func analyzeSpeakerEmbeddingsJSON(audioPath: String, speakersJSON: String) async -> String {
    do {
      guard let data = speakersJSON.data(using: .utf8) else {
        throw SpeechBridgeError.message("Failed to decode speaker analysis request.")
      }

      let speakers = try JSONDecoder().decode([SpeakerEmbeddingRequestPayload].self, from: data)
      let payload = try await speakerEmbeddingPipeline.analyzeAudioFile(
        atPath: audioPath,
        requests: speakers,
        sampleLimit: 3
      )
      return encodeJSON(payload)
    } catch {
      return encodeJSON(
        FileSpeakerEmbeddingPayload(speakers: [], error: error.localizedDescription))
    }
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
    if statusText.isEmpty {
      state.currentFile = "Preparing \(kind.label)..."
    } else {
      state.currentFile = statusText
    }
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

@_cdecl("_speech_model_cache_dir")
public func _speech_model_cache_dir(modelId: SRString) -> SRString {
  SRString(
    waitForValue {
      await SpeechBridge.shared.cacheDirectory(modelId: modelId.toString())
    })
}

@_cdecl("_speech_model_download_state")
public func _speech_model_download_state(modelId: SRString) -> SRString {
  SRString(
    waitForValue {
      await SpeechBridge.shared.modelDownloadStateJSON(modelId: modelId.toString())
    })
}

@_cdecl("_speech_model_start_download")
public func _speech_model_start_download(modelId: SRString) -> Bool {
  waitForValue {
    await SpeechBridge.shared.startModelDownload(modelId: modelId.toString())
    return true
  }
}

@_cdecl("_speech_model_reset")
public func _speech_model_reset(modelId: SRString) -> Bool {
  waitForValue {
    await SpeechBridge.shared.resetModel(modelId: modelId.toString())
    return true
  }
}

@_cdecl("_speech_transcribe_audio_file")
public func _speech_transcribe_audio_file(
  modelId: SRString,
  audioPath: SRString,
  language: SRString
) -> SRString {
  SRString(
    waitForValue {
      await SpeechBridge.shared.transcribeAudioFileJSON(
        modelId: modelId.toString(),
        audioPath: audioPath.toString(),
        language: language.toString()
      )
    })
}

@_cdecl("_speech_diarize_audio_file")
public func _speech_diarize_audio_file(audioPath: SRString, speakerCount: Int) -> SRString {
  SRString(
    waitForValue {
      await SpeechBridge.shared.diarizeAudioFileJSON(
        audioPath: audioPath.toString(),
        speakerCount: speakerCount
      )
    })
}

@_cdecl("_speech_embed_speaker_audio_file")
public func _speech_embed_speaker_audio_file(
  audioPath: SRString,
  speakersJSON: SRString
) -> SRString {
  SRString(
    waitForValue {
      await SpeechBridge.shared.analyzeSpeakerEmbeddingsJSON(
        audioPath: audioPath.toString(),
        speakersJSON: speakersJSON.toString()
      )
    })
}

@_cdecl("_speech_live_transcription_start")
public func _speech_live_transcription_start(
  mode: SRString,
  modelId: SRString,
  recordingPath: SRString,
  language: SRString
) -> SRString {
  SRString(
    waitForValue {
      await SpeechBridge.shared.startTranscriptionJSON(
        mode: mode.toString(),
        modelId: modelId.toString(),
        recordingPath: recordingPath.toString(),
        language: language.toString()
      )
    })
}

@_cdecl("_speech_live_transcription_state")
public func _speech_live_transcription_state() -> SRString {
  SRString(waitForValue { await SpeechBridge.shared.transcriptionStateJSON() })
}

@_cdecl("_speech_live_transcription_request_stop")
public func _speech_live_transcription_request_stop() -> SRString {
  SRString(waitForValue { await SpeechBridge.shared.requestStopTranscriptionJSON() })
}

@_cdecl("_speech_live_transcription_append")
public func _speech_live_transcription_append(
  mixedSamples: SRData,
  microphoneSamples: SRData,
  systemSamples: SRData
) -> SRString {
  SRString(
    waitForValue {
      await SpeechBridge.shared.appendTranscriptionAudio(
        mixedSamplesData: Data(mixedSamples.toArray()),
        microphoneSamplesData: Data(microphoneSamples.toArray()),
        systemSamplesData: Data(systemSamples.toArray())
      )
    })
}

@_cdecl("_speech_live_transcription_stop")
public func _speech_live_transcription_stop() -> SRString {
  SRString(waitForValue { await SpeechBridge.shared.stopTranscriptionJSON() })
}
