import Foundation
import FoundationModels
import SwiftRs

private struct GenerationRequest: Decodable {
  let instructions: String
  let prompt: String
  let maximumResponseTokens: Int?
  let temperature: Double?
  let useGreedySampling: Bool
}

private struct AvailabilityPayload: Encodable {
  let status: String
  let reason: String?
}

private struct GenerationPayload: Encodable {
  let text: String?
  let error: String?
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

private func availabilityPayload() -> AvailabilityPayload {
  guard #available(macOS 26.0, *) else {
    return AvailabilityPayload(status: "unsupported", reason: "unsupported_os")
  }

  switch SystemLanguageModel.default.availability {
  case .available:
    return AvailabilityPayload(status: "available", reason: nil)
  case .unavailable(.deviceNotEligible):
    return AvailabilityPayload(status: "unavailable", reason: "device_not_eligible")
  case .unavailable(.appleIntelligenceNotEnabled):
    return AvailabilityPayload(
      status: "unavailable",
      reason: "apple_intelligence_not_enabled")
  case .unavailable(.modelNotReady):
    return AvailabilityPayload(status: "unavailable", reason: "model_not_ready")
  @unknown default:
    return AvailabilityPayload(status: "unavailable", reason: "unknown")
  }
}

private actor FoundationModelBridge {
  static let shared = FoundationModelBridge()

  func generateJSON(requestJSON: String) async -> String {
    guard #available(macOS 26.0, *) else {
      return encodeJSON(
        GenerationPayload(
          text: nil,
          error: "Apple Foundation Models requires macOS 26 or later."))
    }

    guard case .available = SystemLanguageModel.default.availability else {
      return encodeJSON(
        GenerationPayload(
          text: nil,
          error: "Apple Intelligence is not available on this Mac."))
    }

    do {
      let request = try JSONDecoder().decode(
        GenerationRequest.self,
        from: Data(requestJSON.utf8))
      let instructions = request.instructions.isEmpty ? nil : request.instructions
      let session = LanguageModelSession(instructions: instructions)
      let options = GenerationOptions(
        sampling: request.useGreedySampling ? .greedy : nil,
        temperature: request.useGreedySampling ? nil : request.temperature,
        maximumResponseTokens: request.maximumResponseTokens)
      let response = try await session.respond(to: request.prompt, options: options)

      return encodeJSON(GenerationPayload(text: response.content, error: nil))
    } catch {
      let message =
        (error as? LocalizedError)?.errorDescription
        ?? error.localizedDescription
      return encodeJSON(GenerationPayload(text: nil, error: message))
    }
  }
}

@_cdecl("_foundation_model_availability")
public func _foundationModelAvailability() -> SRString {
  SRString(encodeJSON(availabilityPayload()))
}

@_cdecl("_foundation_model_generate")
public func _foundationModelGenerate(requestJSON: SRString) -> SRString {
  SRString(
    waitForValue {
      await FoundationModelBridge.shared.generateJSON(
        requestJSON: requestJSON.toString())
    })
}
