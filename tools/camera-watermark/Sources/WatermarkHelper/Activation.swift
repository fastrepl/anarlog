import Foundation
import SystemExtensions
import WatermarkKit

/// Submits the Camera Extension inside this app to macOS, or removes it. On activation macOS asks
/// the person to allow it in System Settings → General → Login Items & Extensions → Camera
/// Extensions and reports `needs_approval` until they do. Needs this app under /Applications
/// (inside Anarlog.app there) and the `system-extension.install` entitlement, which only a
/// provisioning profile grants.
final class ExtensionRequest: NSObject, OSSystemExtensionRequestDelegate {
  private let removing: Bool
  private let report: (ExtensionState, String?) -> Void
  /// Requests in flight; the manager holds its delegate weakly.
  private static var inFlight: [ExtensionRequest] = []

  static func submit(removing: Bool, report: @escaping (ExtensionState, String?) -> Void) {
    let delegate = ExtensionRequest(removing: removing, report: report)
    inFlight.append(delegate)
    let request =
      removing
      ? OSSystemExtensionRequest.deactivationRequest(
        forExtensionWithIdentifier: CameraIdentity.extensionBundleID, queue: .main)
      : OSSystemExtensionRequest.activationRequest(
        forExtensionWithIdentifier: CameraIdentity.extensionBundleID, queue: .main)
    request.delegate = delegate
    OSSystemExtensionManager.shared.submitRequest(request)
  }

  private init(removing: Bool, report: @escaping (ExtensionState, String?) -> Void) {
    self.removing = removing
    self.report = report
  }

  private func finish(_ state: ExtensionState, _ detail: String?) {
    report(state, detail)
    Self.inFlight.removeAll { $0 === self }
  }

  func request(
    _ request: OSSystemExtensionRequest,
    actionForReplacingExtension existing: OSSystemExtensionProperties,
    withExtension ext: OSSystemExtensionProperties
  ) -> OSSystemExtensionRequest.ReplacementAction {
    .replace
  }

  func requestNeedsUserApproval(_ request: OSSystemExtensionRequest) {
    // The request stays open: it finishes once the person allows the extension.
    report(.needsApproval, nil)
  }

  func request(
    _ request: OSSystemExtensionRequest,
    didFinishWithResult result: OSSystemExtensionRequest.Result
  ) {
    finish(
      removing ? .removed : .active, result == .willCompleteAfterReboot ? "after_restart" : nil)
  }

  func request(_ request: OSSystemExtensionRequest, didFailWithError error: Error) {
    finish(.failed, Self.describe(error))
  }

  static func describe(_ error: Error) -> String {
    let code = (error as NSError).code
    switch OSSystemExtensionError.Code(rawValue: code) {
    case .missingEntitlement: return "missing_entitlement"
    case .unsupportedParentBundleLocation: return "not_in_applications"
    case .extensionNotFound: return "extension_not_found"
    case .codeSignatureInvalid: return "code_signature_invalid"
    case .validationFailed: return "validation_failed"
    case .requestCanceled: return "canceled"
    case .authorizationRequired: return "authorization_required"
    case .forbiddenBySystemPolicy: return "forbidden_by_policy"
    default: return "error_\(code)"
    }
  }
}
