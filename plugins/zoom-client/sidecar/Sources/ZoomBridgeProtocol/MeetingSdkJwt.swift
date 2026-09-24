import CryptoKit
import Foundation

/// Mints the HS256 auth token the Zoom Meeting SDK expects
/// (https://developers.zoom.us/docs/meeting-sdk/auth/). Credentials only ever arrive via
/// environment; nothing here persists them.
public enum MeetingSdkJwt {
  public struct Credentials {
    public var key: String
    public var secret: String

    public init(key: String, secret: String) {
      self.key = key
      self.secret = secret
    }

    public static func fromEnvironment(
      _ env: [String: String] = ProcessInfo.processInfo.environment
    ) -> Credentials? {
      guard let key = env["ANARLOG_ZOOM_SDK_KEY"], let secret = env["ANARLOG_ZOOM_SDK_SECRET"],
        !key.isEmpty, !secret.isEmpty
      else { return nil }
      return Credentials(key: key, secret: secret)
    }
  }

  public static func mint(
    _ credentials: Credentials, meetingNumber: Int64, role: Int = 0,
    now: Date = Date(), lifetime: TimeInterval = 2 * 60 * 60
  ) -> String {
    let iat = Int(now.timeIntervalSince1970) - 30
    let exp = iat + Int(lifetime)
    let header = base64url(#"{"alg":"HS256","typ":"JWT"}"#.data(using: .utf8)!)
    let payload = base64url(
      """
      {"appKey":"\(credentials.key)","sdkKey":"\(credentials.key)","mn":"\(meetingNumber)",\
      "role":\(role),"iat":\(iat),"exp":\(exp),"tokenExp":\(exp)}
      """.data(using: .utf8)!)
    let signingInput = "\(header).\(payload)"
    let signature = HMAC<SHA256>.authenticationCode(
      for: signingInput.data(using: .utf8)!,
      using: SymmetricKey(data: credentials.secret.data(using: .utf8)!))
    return "\(signingInput).\(base64url(Data(signature)))"
  }

  static func base64url(_ data: Data) -> String {
    data.base64EncodedString()
      .replacingOccurrences(of: "+", with: "-")
      .replacingOccurrences(of: "/", with: "_")
      .replacingOccurrences(of: "=", with: "")
  }
}
