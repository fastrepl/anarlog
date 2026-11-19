import Foundation

/// Who may write frames into Anarlog Camera's sink. Without this any local process could open the
/// sink and replace the picture others see, watermark included. Only `Anarlog Camera.app`, signed
/// by the same team that signed the extension, may write, and one writer at a time.
///
/// The team is the extension's own (`SignedClient.teamID(pid: getpid())`), not a constant: the
/// repository carries no team ID, and an extension that isn't signed by a team (an ad-hoc dev
/// build) admits nobody.
public struct SinkGate: Sendable {
  public static var helperSigningID: String { CameraIdentity.helperBundleID }

  public enum Decision: Equatable, Sendable {
    case accept
    /// Not the helper, by signing identifier.
    case notHelper
    /// Says it is the helper but isn't signed by the extension's team, or the extension has none.
    case notOurTeam
    /// Another helper is already writing.
    case busy
  }

  /// The team the extension is signed by; nil refuses every writer.
  public let teamID: String?
  public private(set) var writer: UUID?

  public init(teamID: String?) {
    self.teamID = teamID.flatMap { $0.isEmpty ? nil : $0 }
  }

  /// Whether `client` may start writing. `clientTeamID` is the team its signature was verified
  /// against (`SignedClient.verifiedTeam`), nil when it couldn't be verified.
  public mutating func admit(client: UUID, signingID: String?, clientTeamID: String?) -> Decision {
    guard signingID == Self.helperSigningID else { return .notHelper }
    guard let teamID, clientTeamID == teamID else { return .notOurTeam }
    if let writer, writer != client { return .busy }
    writer = client
    return .accept
  }

  /// The writer stopped or went away; nil when CMIO doesn't say who (`stopStream`).
  public mutating func release(_ client: UUID?) {
    if client == nil || client == writer { writer = nil }
  }
}

/// A connected client's code signature as the kernel recorded it when the process started, not
/// what the client says about itself. CMIO reports `signingID` as "unknown" for Developer ID
/// clients, and a CMIO extension's sandbox refuses both `SecCodeCopyGuestWithAttributes` (EPERM)
/// and reading binaries under /Applications, so this asks the kernel through `csops`, which the
/// sandbox allows. The kernel holds the identifier and team from the code directory of a valid,
/// non-ad-hoc signature. CMIO gives no audit token, so a pid reused between connect and this
/// check is accepted: the new process would still need the helper's identifier and team.
public enum SignedClient {
  /// The signing identifier of the process `pid` (e.g. "us.zoom.xos"), nil when it has no valid
  /// signature.
  public static func signingID(pid: pid_t) -> String? {
    guard isValidlySigned(pid: pid) else { return nil }
    return string(pid: pid, op: csOpsIdentity)
  }

  /// The team of a validly signed, non-ad-hoc process `pid`; nil for Apple's own binaries, ad-hoc
  /// signatures and unsigned code.
  public static func teamID(pid: pid_t) -> String? {
    guard isValidlySigned(pid: pid) else { return nil }
    return string(pid: pid, op: csOpsTeamID)
  }

  /// `team` when the process `pid` runs the helper's own binary, validly signed and not ad hoc,
  /// with the hardened runtime, under `team`; else nil.
  public static func verifiedTeam(pid: pid_t, expected team: String?) -> String? {
    // Notarized code always runs with the hardened runtime.
    guard let team, isValidlySigned(pid: pid, requiring: csRuntime),
      string(pid: pid, op: csOpsIdentity) == SinkGate.helperSigningID,
      string(pid: pid, op: csOpsTeamID) == team
    else { return nil }
    return team
  }

  private static let csOpsStatus: UInt32 = 0
  private static let csOpsIdentity: UInt32 = 11
  private static let csOpsTeamID: UInt32 = 14
  private static let csValid: UInt32 = 0x1
  private static let csAdhoc: UInt32 = 0x2
  private static let csRuntime: UInt32 = 0x10000
  private static let csLinkerSigned: UInt32 = 0x20000

  private static func isValidlySigned(pid: pid_t, requiring required: UInt32 = 0) -> Bool {
    var flags: UInt32 = 0
    guard csops(pid, csOpsStatus, &flags, MemoryLayout<UInt32>.size) == 0 else { return false }
    return flags & csValid != 0 && flags & (csAdhoc | csLinkerSigned) == 0
      && flags & required == required
  }

  /// `CS_OPS_IDENTITY` and `CS_OPS_TEAMID` answer an 8-byte blob header, then the string.
  private static func string(pid: pid_t, op: UInt32) -> String? {
    var buffer = [UInt8](repeating: 0, count: 1024)
    guard csops(pid, op, &buffer, buffer.count) == 0 else { return nil }
    let text = String(decoding: buffer.dropFirst(8).prefix { $0 != 0 }, as: UTF8.self)
    return text.isEmpty ? nil : text
  }
}

/// `csops(2)` from libsystem_kernel; its header isn't in the public SDK.
@_silgen_name("csops")
private func csops(
  _ pid: pid_t, _ ops: UInt32, _ useraddr: UnsafeMutableRawPointer?, _ usersize: Int
)
  -> Int32
