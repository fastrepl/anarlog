import AVFoundation
import Foundation
import Speech
import SwiftRs

private enum SpeechAnalyzerBridgeError: LocalizedError {
  case message(String)

  var errorDescription: String? {
    switch self {
    case .message(let message):
      return message
    }
  }
}

private let liveSampleRate = 16_000.0

private enum TranscriptSource: String, Codable, CaseIterable {
  case microphone
  case system
}

private struct AvailabilityPayload: Codable {
  var status: String
  var reason: String?
}

private struct LocalesPayload: Codable {
  var locales: [String]
  var error: String?
}

private struct DownloadStatePayload: Codable {
  var status: String
  var currentFile: String?
  var progressPercent: Int?
  var localPath: String
  var error: String?
}

private struct StatusPayload: Codable {
  var running: Bool
  var error: String?
}

private struct WordPayload: Codable {
  var text: String
  var start: Double
  var end: Double
  var confidence: Double?
}

private struct LivePartialPayload: Codable {
  var source: String
  var text: String
  var isFinal: Bool
  var start: Double
  var end: Double
  var words: [WordPayload]
}

private struct LiveAppendPayload: Codable {
  var partials: [LivePartialPayload]
  var error: String?
}

private struct FileTranscriptionPayload: Codable {
  var text: String
  var durationSeconds: Double
  var words: [WordPayload]
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
    throw SpeechAnalyzerBridgeError.message("Invalid audio chunk received by SpeechAnalyzer.")
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

@available(macOS 26.0, *)
private func makeTranscriber(locale: Locale) -> SpeechTranscriber {
  SpeechTranscriber(
    locale: locale,
    transcriptionOptions: [],
    reportingOptions: [.volatileResults],
    attributeOptions: [.audioTimeRange, .transcriptionConfidence]
  )
}

/// `supportedLocale(equivalentTo:)` echoes unknown tags back rather than returning nil
/// (it answers `nb-NO` for Norwegian, which Apple Speech does not transcribe), so its
/// answer is only trusted after checking it against `supportedLocales`.
@available(macOS 26.0, *)
private func matchSupportedLocale(_ requested: Locale) async -> Locale? {
  guard let candidate = await SpeechTranscriber.supportedLocale(equivalentTo: requested) else {
    return nil
  }

  let supported = await SpeechTranscriber.supportedLocales
  let tag = candidate.identifier(.bcp47).lowercased()
  return supported.contains { $0.identifier(.bcp47).lowercased() == tag } ? candidate : nil
}

@available(macOS 26.0, *)
private func resolveLocale(_ identifier: String) async throws -> Locale {
  let trimmed = identifier.trimmingCharacters(in: .whitespacesAndNewlines)
  let requested = Locale(identifier: trimmed.isEmpty ? "en-US" : trimmed)

  guard let supported = await matchSupportedLocale(requested) else {
    throw SpeechAnalyzerBridgeError.message(
      "Apple Speech does not support \(requested.identifier(.bcp47)).")
  }

  return supported
}

/// The languages the user has explicitly added in System Settings, narrowed to what
/// Apple Speech can transcribe. This is the set the app is allowed to install.
@available(macOS 26.0, *)
private func preferredSupportedLocales() async -> [String] {
  var resolved: [String] = []

  for tag in Locale.preferredLanguages {
    guard let match = await matchSupportedLocale(Locale(identifier: tag)) else { continue }
    let identifier = match.identifier(.bcp47)
    if !resolved.contains(identifier) {
      resolved.append(identifier)
    }
  }

  return resolved
}

/// `maximumReservedLocales` is a hard cap, so make room by releasing a locale
/// that is not the one being installed.
@available(macOS 26.0, *)
private func reserveSupportedLocale(_ locale: Locale) async throws {
  let reserved = await AssetInventory.reservedLocales
  if reserved.contains(where: { $0.identifier(.bcp47) == locale.identifier(.bcp47) }) {
    return
  }

  if reserved.count >= AssetInventory.maximumReservedLocales,
    let evictable = reserved.first(where: {
      $0.identifier(.bcp47) != locale.identifier(.bcp47)
    })
  {
    _ = await AssetInventory.release(reservedLocale: evictable)
  }

  _ = try await AssetInventory.reserve(locale: locale)
}

@available(macOS 26.0, *)
private func reportedProgressPercent(_ progress: Progress) -> Int? {
  guard !progress.isIndeterminate, progress.totalUnitCount > 0, progress.fractionCompleted > 0
  else {
    return nil
  }

  return Int(min(1.0, progress.fractionCompleted) * 100.0)
}

/// Installs assets off the bridge actor. `downloadAndInstall()` can return after a
/// queued first attempt, so we keep watching `AssetInventory.status` until the
/// locale is actually installed.
@available(macOS 26.0, *)
private func installLocaleAssets(key: String, identifier: String) async throws {
  let locale = try await resolveLocale(identifier)
  let tag = locale.identifier(.bcp47)
  let preferred = await preferredSupportedLocales()
  guard preferred.contains(where: { $0.compare(tag, options: [.caseInsensitive]) == .orderedSame })
  else {
    throw SpeechAnalyzerBridgeError.message(
      "Apple Speech can only install languages added in System Settings.")
  }

  let transcriber = makeTranscriber(locale: locale)
  try await reserveSupportedLocale(locale)

  if let request = try await AssetInventory.assetInstallationRequest(supporting: [transcriber]) {
    let progressTask = Task.detached(priority: .utility) { [progress = request.progress] in
      while !Task.isCancelled && !progress.isFinished {
        await SpeechAnalyzerBridge.shared.updateDownloadProgress(
          key: key, percent: reportedProgressPercent(progress))
        try? await Task.sleep(nanoseconds: 300_000_000)
      }
    }

    try await request.downloadAndInstall()
    progressTask.cancel()
  }

  for _ in 0..<3600 {
    try Task.checkCancellation()
    switch await AssetInventory.status(forModules: [transcriber]) {
    case .installed:
      return
    case .unsupported:
      throw SpeechAnalyzerBridgeError.message("Apple Speech does not support \(tag).")
    case .downloading, .supported:
      try await Task.sleep(nanoseconds: 500_000_000)
    @unknown default:
      try await Task.sleep(nanoseconds: 500_000_000)
    }
  }

  throw SpeechAnalyzerBridgeError.message("Apple Speech asset install timed out.")
}

/// Volatile results carry a single run spanning the whole hypothesis, so per-word
/// timings only materialize on finalized results.
@available(macOS 26.0, *)
private func words(from text: AttributedString) -> [WordPayload] {
  var payloads: [WordPayload] = []

  for run in text.runs {
    guard let range = run.audioTimeRange else { continue }
    let word = String(text[run.range].characters).trimmingCharacters(in: .whitespacesAndNewlines)
    guard !word.isEmpty else { continue }

    payloads.append(
      WordPayload(
        text: word,
        start: range.start.seconds,
        end: (range.start + range.duration).seconds,
        confidence: run.transcriptionConfidence
      )
    )
  }

  return payloads
}

@available(macOS 26.0, *)
private func partial(from result: SpeechTranscriber.Result, source: TranscriptSource)
  -> LivePartialPayload
{
  LivePartialPayload(
    source: source.rawValue,
    text: String(result.text.characters),
    isFinal: result.isFinal,
    start: result.range.start.seconds,
    end: (result.range.start + result.range.duration).seconds,
    words: result.isFinal ? words(from: result.text) : []
  )
}

@available(macOS 26.0, *)
private final class LiveSession {
  let analyzer: SpeechAnalyzer
  let continuation: AsyncStream<AnalyzerInput>.Continuation
  let analyzerFormat: AVAudioFormat
  let converter: AVAudioConverter?
  let sourceFormat: AVAudioFormat
  var collector: Task<Void, Never>?
  var finished = false

  init(
    analyzer: SpeechAnalyzer,
    continuation: AsyncStream<AnalyzerInput>.Continuation,
    analyzerFormat: AVAudioFormat,
    sourceFormat: AVAudioFormat,
    converter: AVAudioConverter?
  ) {
    self.analyzer = analyzer
    self.continuation = continuation
    self.analyzerFormat = analyzerFormat
    self.sourceFormat = sourceFormat
    self.converter = converter
  }

  func buffer(for samples: [Float]) -> AVAudioPCMBuffer? {
    guard !samples.isEmpty,
      let input = AVAudioPCMBuffer(
        pcmFormat: sourceFormat, frameCapacity: AVAudioFrameCount(samples.count)),
      let channel = input.floatChannelData
    else {
      return nil
    }

    input.frameLength = AVAudioFrameCount(samples.count)
    samples.withUnsafeBufferPointer { source in
      channel[0].update(from: source.baseAddress!, count: samples.count)
    }

    guard let converter else { return input }

    let capacity =
      AVAudioFrameCount(
        Double(input.frameLength) * analyzerFormat.sampleRate / sourceFormat.sampleRate) + 4096
    guard let output = AVAudioPCMBuffer(pcmFormat: analyzerFormat, frameCapacity: capacity) else {
      return nil
    }

    var consumed = false
    var error: NSError?
    let status = converter.convert(to: output, error: &error) { _, outStatus in
      if consumed {
        outStatus.pointee = .noDataNow
        return nil
      }
      consumed = true
      outStatus.pointee = .haveData
      return input
    }

    guard error == nil, status != .error, output.frameLength > 0 else { return nil }
    return output
  }
}

private actor SpeechAnalyzerBridge {
  static let shared = SpeechAnalyzerBridge()

  private var pending: [TranscriptSource: [LivePartialPayload]] = [:]
  private var downloadStates: [String: DownloadStatePayload] = [:]
  private var downloadTasks: [String: Task<Void, Never>] = [:]

  // Sessions are only reachable behind a macOS 26 availability check; `Any` keeps the
  // stored-property type off the older deployment target.
  private var sessions: [TranscriptSource: Any] = [:]

  // MARK: Availability

  func availabilityJSON() -> String {
    guard #available(macOS 26.0, *) else {
      return encodeJSON(AvailabilityPayload(status: "unsupported", reason: "unsupported_os"))
    }

    guard SpeechTranscriber.isAvailable else {
      return encodeJSON(AvailabilityPayload(status: "unavailable", reason: "device_not_eligible"))
    }

    return encodeJSON(AvailabilityPayload(status: "available", reason: nil))
  }

  func supportedLocalesJSON() async -> String {
    guard #available(macOS 26.0, *) else {
      return encodeJSON(LocalesPayload(locales: [], error: "unsupported_os"))
    }

    let locales = await SpeechTranscriber.supportedLocales
    return encodeJSON(LocalesPayload(locales: locales.map { $0.identifier(.bcp47) }, error: nil))
  }

  func preferredLocalesJSON() async -> String {
    guard #available(macOS 26.0, *) else {
      return encodeJSON(LocalesPayload(locales: [], error: "unsupported_os"))
    }

    return encodeJSON(LocalesPayload(locales: await preferredSupportedLocales(), error: nil))
  }

  func installedLocalesJSON() async -> String {
    guard #available(macOS 26.0, *) else {
      return encodeJSON(LocalesPayload(locales: [], error: "unsupported_os"))
    }

    let locales = await SpeechTranscriber.installedLocales
    return encodeJSON(LocalesPayload(locales: locales.map { $0.identifier(.bcp47) }, error: nil))
  }

  // MARK: Assets

  func downloadStateJSON(locale identifier: String) async -> String {
    guard #available(macOS 26.0, *) else {
      return encodeJSON(
        DownloadStatePayload(
          status: "error", currentFile: nil, progressPercent: nil, localPath: "",
          error: "Apple Speech requires macOS 26 or newer."))
    }

    let key = normalizedKey(identifier)

    do {
      let locale = try await resolveLocale(identifier)
      let transcriber = makeTranscriber(locale: locale)
      let status = await AssetInventory.status(forModules: [transcriber])

      var state = downloadState(for: key)
      switch status {
      case .installed:
        state.status = "ready"
        state.error = nil
        state.currentFile = nil
        state.progressPercent = nil
      case .downloading:
        state.status = "downloading"
      case .supported:
        if downloadTasks[key] != nil {
          state.status = "downloading"
        } else {
          state.status = "idle"
        }
      case .unsupported:
        state.status = "error"
        state.error = "Apple Speech does not support this language."
      @unknown default:
        state.status = "idle"
      }
      downloadStates[key] = state
      return encodeJSON(state)
    } catch {
      var state = downloadState(for: key)
      state.status = "error"
      state.error = error.localizedDescription
      downloadStates[key] = state
      return encodeJSON(state)
    }
  }

  func startDownload(locale identifier: String) {
    guard #available(macOS 26.0, *) else { return }

    let key = normalizedKey(identifier)
    guard downloadTasks[key] == nil else { return }

    var state = downloadState(for: key)
    state.status = "downloading"
    state.currentFile = "Preparing \(key)..."
    state.progressPercent = nil
    state.error = nil
    downloadStates[key] = state

    // AssetInventory work stays off the actor so progress polls can hop back in.
    let task = Task.detached(priority: .utility) {
      do {
        try await installLocaleAssets(key: key, identifier: identifier)
        await SpeechAnalyzerBridge.shared.finishDownload(key: key, error: nil)
      } catch {
        await SpeechAnalyzerBridge.shared.finishDownload(
          key: key, error: error.localizedDescription)
      }
    }
    downloadTasks[key] = task
  }

  func updateDownloadProgress(key: String, percent: Int?) {
    var state = downloadState(for: key)
    state.status = "downloading"
    if let percent {
      state.progressPercent = percent
    }
    state.currentFile = "Downloading \(key)..."
    state.error = nil
    downloadStates[key] = state
  }

  private func finishDownload(key: String, error: String?) {
    downloadTasks[key] = nil

    var state = downloadState(for: key)
    state.currentFile = nil
    state.progressPercent = nil
    if let error {
      state.status = "error"
      state.error = error
    } else {
      state.status = "ready"
      state.error = nil
    }
    downloadStates[key] = state
  }

  func releaseLocale(_ identifier: String) async {
    guard #available(macOS 26.0, *) else { return }

    let key = normalizedKey(identifier)
    downloadTasks[key]?.cancel()
    downloadTasks[key] = nil
    downloadStates[key] = nil

    guard let locale = try? await resolveLocale(identifier) else { return }
    _ = await AssetInventory.release(reservedLocale: locale)
  }

  private func normalizedKey(_ identifier: String) -> String {
    let trimmed = identifier.trimmingCharacters(in: .whitespacesAndNewlines)
    return trimmed.isEmpty ? "en-US" : trimmed
  }

  private func downloadState(for key: String) -> DownloadStatePayload {
    downloadStates[key]
      ?? DownloadStatePayload(
        status: "idle", currentFile: nil, progressPercent: nil, localPath: "", error: nil)
  }

  // MARK: Live transcription

  func startLiveJSON(locale identifier: String) async -> String {
    guard #available(macOS 26.0, *) else {
      return encodeJSON(
        StatusPayload(running: false, error: "Apple Speech requires macOS 26 or newer."))
    }

    await teardownSessions()

    do {
      let locale = try await resolveLocale(identifier)
      for source in TranscriptSource.allCases {
        sessions[source] = try await makeSession(locale: locale, source: source)
        pending[source] = []
      }
      return encodeJSON(StatusPayload(running: true, error: nil))
    } catch {
      await teardownSessions()
      return encodeJSON(StatusPayload(running: false, error: error.localizedDescription))
    }
  }

  @available(macOS 26.0, *)
  private func makeSession(locale: Locale, source: TranscriptSource) async throws -> LiveSession {
    let transcriber = makeTranscriber(locale: locale)
    try await ensureAssetsInstalled(locale: locale, transcriber: transcriber)

    guard
      let analyzerFormat = await SpeechAnalyzer.bestAvailableAudioFormat(
        compatibleWith: [transcriber])
    else {
      throw SpeechAnalyzerBridgeError.message("No compatible Apple Speech audio format.")
    }

    guard
      let sourceFormat = AVAudioFormat(
        commonFormat: .pcmFormatFloat32, sampleRate: liveSampleRate, channels: 1, interleaved: false
      )
    else {
      throw SpeechAnalyzerBridgeError.message("Failed to build Apple Speech input format.")
    }

    let converter =
      sourceFormat.isEqual(analyzerFormat)
      ? nil : AVAudioConverter(from: sourceFormat, to: analyzerFormat)
    if converter == nil && !sourceFormat.isEqual(analyzerFormat) {
      throw SpeechAnalyzerBridgeError.message("Failed to build Apple Speech audio converter.")
    }

    let analyzer = SpeechAnalyzer(modules: [transcriber])
    let (stream, continuation) = AsyncStream<AnalyzerInput>.makeStream()
    let session = LiveSession(
      analyzer: analyzer,
      continuation: continuation,
      analyzerFormat: analyzerFormat,
      sourceFormat: sourceFormat,
      converter: converter
    )

    session.collector = Task.detached(priority: .userInitiated) {
      do {
        for try await result in transcriber.results {
          await SpeechAnalyzerBridge.shared.collect(
            partial(from: result, source: source), for: source)
        }
      } catch {
        await SpeechAnalyzerBridge.shared.collectError(
          error.localizedDescription, for: source)
      }
    }

    try await analyzer.start(inputSequence: stream)
    return session
  }

  @available(macOS 26.0, *)
  private func ensureAssetsInstalled(locale: Locale, transcriber: SpeechTranscriber) async throws {
    try await reserveSupportedLocale(locale)

    if let request = try await AssetInventory.assetInstallationRequest(supporting: [transcriber]) {
      try await request.downloadAndInstall()
    }

    guard await AssetInventory.status(forModules: [transcriber]) == .installed else {
      throw SpeechAnalyzerBridgeError.message(
        "Apple Speech assets for \(locale.identifier(.bcp47)) are not installed yet.")
    }
  }

  private func collect(_ payload: LivePartialPayload, for source: TranscriptSource) {
    pending[source, default: []].append(payload)
  }

  private func collectError(_ message: String, for source: TranscriptSource) {
    tracingErrors[source] = message
  }

  private var tracingErrors: [TranscriptSource: String] = [:]

  func appendLiveJSON(source rawSource: String, samplesData: Data) async -> String {
    guard #available(macOS 26.0, *) else {
      return encodeJSON(
        LiveAppendPayload(partials: [], error: "Apple Speech requires macOS 26 or newer."))
    }

    do {
      guard let source = TranscriptSource(rawValue: rawSource) else {
        throw SpeechAnalyzerBridgeError.message(
          "Unsupported Apple Speech transcript source: \(rawSource)")
      }
      guard let session = sessions[source] as? LiveSession else {
        throw SpeechAnalyzerBridgeError.message("No active Apple Speech transcription session.")
      }
      if let message = tracingErrors[source] {
        tracingErrors[source] = nil
        throw SpeechAnalyzerBridgeError.message(message)
      }

      let samples = try decodeFloatSamples(from: samplesData)
      if let buffer = session.buffer(for: samples) {
        session.continuation.yield(AnalyzerInput(buffer: buffer))
      }

      return encodeJSON(LiveAppendPayload(partials: drain(source), error: nil))
    } catch {
      return encodeJSON(LiveAppendPayload(partials: [], error: error.localizedDescription))
    }
  }

  func finalizeLiveJSON(source rawSource: String) async -> String {
    guard #available(macOS 26.0, *) else {
      return encodeJSON(
        LiveAppendPayload(partials: [], error: "Apple Speech requires macOS 26 or newer."))
    }

    do {
      guard let source = TranscriptSource(rawValue: rawSource) else {
        throw SpeechAnalyzerBridgeError.message(
          "Unsupported Apple Speech transcript source: \(rawSource)")
      }
      guard let session = sessions[source] as? LiveSession else {
        throw SpeechAnalyzerBridgeError.message("No active Apple Speech transcription session.")
      }

      if !session.finished {
        session.finished = true
        session.continuation.finish()
        try await session.analyzer.finalizeAndFinishThroughEndOfInput()
        await session.collector?.value
      }

      return encodeJSON(LiveAppendPayload(partials: drain(source), error: nil))
    } catch {
      return encodeJSON(LiveAppendPayload(partials: [], error: error.localizedDescription))
    }
  }

  func stopLiveJSON() async -> String {
    await teardownSessions()
    return encodeJSON(StatusPayload(running: false, error: nil))
  }

  private func drain(_ source: TranscriptSource) -> [LivePartialPayload] {
    let partials = pending[source] ?? []
    pending[source] = []
    return partials
  }

  private func teardownSessions() async {
    guard #available(macOS 26.0, *) else {
      sessions = [:]
      pending = [:]
      tracingErrors = [:]
      return
    }

    for value in sessions.values {
      guard let session = value as? LiveSession else { continue }
      if !session.finished {
        session.finished = true
        session.continuation.finish()
        await session.analyzer.cancelAndFinishNow()
      }
      session.collector?.cancel()
    }

    sessions = [:]
    pending = [:]
    tracingErrors = [:]
  }

  // MARK: File transcription

  /// Batch transcription of raw 16kHz mono samples. The batch path splits a recording
  /// into per-source channels before transcribing, so it cannot go through a file URL.
  func transcribeSamplesJSON(samplesData: Data, locale identifier: String) async -> String {
    guard #available(macOS 26.0, *) else {
      return encodeJSON(
        FileTranscriptionPayload(
          text: "", durationSeconds: 0, words: [],
          error: "Apple Speech requires macOS 26 or newer."))
    }

    do {
      let samples = try decodeFloatSamples(from: samplesData)
      let duration = Double(samples.count) / liveSampleRate

      guard !samples.isEmpty else {
        return encodeJSON(
          FileTranscriptionPayload(text: "", durationSeconds: 0, words: [], error: nil))
      }

      let locale = try await resolveLocale(identifier)
      let transcriber = makeTranscriber(locale: locale)
      try await ensureAssetsInstalled(locale: locale, transcriber: transcriber)

      guard
        let analyzerFormat = await SpeechAnalyzer.bestAvailableAudioFormat(
          compatibleWith: [transcriber]),
        let sourceFormat = AVAudioFormat(
          commonFormat: .pcmFormatFloat32, sampleRate: liveSampleRate, channels: 1,
          interleaved: false)
      else {
        throw SpeechAnalyzerBridgeError.message("No compatible Apple Speech audio format.")
      }

      let converter =
        sourceFormat.isEqual(analyzerFormat)
        ? nil : AVAudioConverter(from: sourceFormat, to: analyzerFormat)
      let analyzer = SpeechAnalyzer(modules: [transcriber])
      let (stream, continuation) = AsyncStream<AnalyzerInput>.makeStream()
      let session = LiveSession(
        analyzer: analyzer,
        continuation: continuation,
        analyzerFormat: analyzerFormat,
        sourceFormat: sourceFormat,
        converter: converter
      )

      let collector = Task<([String], [WordPayload]), Error> {
        var texts: [String] = []
        var allWords: [WordPayload] = []
        for try await result in transcriber.results where result.isFinal {
          texts.append(String(result.text.characters))
          allWords.append(contentsOf: words(from: result.text))
        }
        return (texts, allWords)
      }

      try await analyzer.start(inputSequence: stream)

      for chunk in stride(from: 0, to: samples.count, by: 16_000) {
        let slice = Array(samples[chunk..<min(chunk + 16_000, samples.count)])
        if let buffer = session.buffer(for: slice) {
          continuation.yield(AnalyzerInput(buffer: buffer))
        }
      }

      continuation.finish()
      try await analyzer.finalizeAndFinishThroughEndOfInput()
      let (texts, allWords) = try await collector.value

      return encodeJSON(
        FileTranscriptionPayload(
          text: texts.joined(separator: " ").trimmingCharacters(in: .whitespacesAndNewlines),
          durationSeconds: duration,
          words: allWords,
          error: nil
        )
      )
    } catch {
      return encodeJSON(
        FileTranscriptionPayload(
          text: "", durationSeconds: 0, words: [], error: error.localizedDescription))
    }
  }

  func transcribeFileJSON(audioPath: String, locale identifier: String) async -> String {
    guard #available(macOS 26.0, *) else {
      return encodeJSON(
        FileTranscriptionPayload(
          text: "", durationSeconds: 0, words: [],
          error: "Apple Speech requires macOS 26 or newer."))
    }

    do {
      let locale = try await resolveLocale(identifier)
      let transcriber = makeTranscriber(locale: locale)
      try await ensureAssetsInstalled(locale: locale, transcriber: transcriber)

      let file = try AVAudioFile(forReading: URL(fileURLWithPath: audioPath))
      let duration = Double(file.length) / file.processingFormat.sampleRate
      let analyzer = SpeechAnalyzer(modules: [transcriber])

      let collector = Task<([String], [WordPayload]), Error> {
        var texts: [String] = []
        var allWords: [WordPayload] = []
        for try await result in transcriber.results where result.isFinal {
          texts.append(String(result.text.characters))
          allWords.append(contentsOf: words(from: result.text))
        }
        return (texts, allWords)
      }

      _ = try await analyzer.analyzeSequence(from: file)
      try await analyzer.finalizeAndFinishThroughEndOfInput()
      let (texts, allWords) = try await collector.value

      return encodeJSON(
        FileTranscriptionPayload(
          text: texts.joined(separator: " ").trimmingCharacters(in: .whitespacesAndNewlines),
          durationSeconds: duration,
          words: allWords,
          error: nil
        )
      )
    } catch {
      return encodeJSON(
        FileTranscriptionPayload(
          text: "", durationSeconds: 0, words: [], error: error.localizedDescription))
    }
  }
}

// MARK: - C bridge

@_cdecl("_apple_speech_availability")
public func _appleSpeechAvailability() -> SRString {
  SRString(waitForValue { await SpeechAnalyzerBridge.shared.availabilityJSON() })
}

@_cdecl("_apple_speech_supported_locales")
public func _appleSpeechSupportedLocales() -> SRString {
  SRString(waitForValue { await SpeechAnalyzerBridge.shared.supportedLocalesJSON() })
}

@_cdecl("_apple_speech_preferred_locales")
public func _appleSpeechPreferredLocales() -> SRString {
  SRString(waitForValue { await SpeechAnalyzerBridge.shared.preferredLocalesJSON() })
}

@_cdecl("_apple_speech_installed_locales")
public func _appleSpeechInstalledLocales() -> SRString {
  SRString(waitForValue { await SpeechAnalyzerBridge.shared.installedLocalesJSON() })
}

@_cdecl("_apple_speech_download_state")
public func _appleSpeechDownloadState(locale: SRString) -> SRString {
  SRString(
    waitForValue { await SpeechAnalyzerBridge.shared.downloadStateJSON(locale: locale.toString()) })
}

@_cdecl("_apple_speech_start_download")
public func _appleSpeechStartDownload(locale: SRString) -> Bool {
  waitForValue {
    await SpeechAnalyzerBridge.shared.startDownload(locale: locale.toString())
    return true
  }
}

@_cdecl("_apple_speech_release_locale")
public func _appleSpeechReleaseLocale(locale: SRString) -> Bool {
  waitForValue {
    await SpeechAnalyzerBridge.shared.releaseLocale(locale.toString())
    return true
  }
}

@_cdecl("_apple_speech_transcribe_samples")
public func _appleSpeechTranscribeSamples(samples: SRData, locale: SRString) -> SRString {
  SRString(
    waitForValue {
      await SpeechAnalyzerBridge.shared.transcribeSamplesJSON(
        samplesData: Data(samples.toArray()), locale: locale.toString())
    })
}

@_cdecl("_apple_speech_transcribe_audio_file")
public func _appleSpeechTranscribeAudioFile(audioPath: SRString, locale: SRString) -> SRString {
  SRString(
    waitForValue {
      await SpeechAnalyzerBridge.shared.transcribeFileJSON(
        audioPath: audioPath.toString(), locale: locale.toString())
    })
}

@_cdecl("_apple_speech_live_start")
public func _appleSpeechLiveStart(locale: SRString) -> SRString {
  SRString(
    waitForValue { await SpeechAnalyzerBridge.shared.startLiveJSON(locale: locale.toString()) })
}

@_cdecl("_apple_speech_live_append")
public func _appleSpeechLiveAppend(source: SRString, samples: SRData) -> SRString {
  SRString(
    waitForValue {
      await SpeechAnalyzerBridge.shared.appendLiveJSON(
        source: source.toString(), samplesData: Data(samples.toArray()))
    })
}

@_cdecl("_apple_speech_live_finalize")
public func _appleSpeechLiveFinalize(source: SRString) -> SRString {
  SRString(
    waitForValue { await SpeechAnalyzerBridge.shared.finalizeLiveJSON(source: source.toString()) })
}

@_cdecl("_apple_speech_live_stop")
public func _appleSpeechLiveStop() -> SRString {
  SRString(waitForValue { await SpeechAnalyzerBridge.shared.stopLiveJSON() })
}
