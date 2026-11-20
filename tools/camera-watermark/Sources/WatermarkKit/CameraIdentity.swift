import Foundation

/// What the extension, the helper and Anarlog agree on. Every name and ID here is Anarlog's own, so
/// Anarlog Camera installs, registers and runs beside Char Camera without either replacing the
/// other. The helper finds the virtual camera by its UUID and reads who uses it from a custom
/// device property.
public enum CameraIdentity {
  /// The camera's name in every meeting app's camera list. Anarlog switches to it by this exact
  /// name (`CAMERA_NAME` in `plugins/camera-watermark`).
  public static let deviceName = "Anarlog Camera"
  public static let manufacturer = "Anarlog"
  public static let deviceUUID = UUID(uuidString: "78AA3D1F-B9DC-489A-BADC-CEBC44DC663A")!
  public static let sourceStreamUUID = UUID(uuidString: "07D66AE4-213E-4580-B02F-E11F50F1C3A8")!
  public static let sinkStreamUUID = UUID(uuidString: "36C4BA44-F80D-4876-A513-A5B0C1280E33")!

  /// `CFBundleIdentifier` of `Anarlog Camera.app`, and the signing identifier the extension
  /// requires of its one writer (`APP_ID` in `scripts/bundle.mjs`).
  public static let helperBundleID = "so.anarlog.camera"
  /// Must match `CFBundleIdentifier` in the extension's Info.plist (`scripts/bundle.mjs`).
  public static let extensionBundleID = "so.anarlog.camera.extension"

  /// The apps streaming from the camera, as newline-separated signing IDs. A custom CMIO
  /// property: `4cc_<selector>_<scope>_<element>`. Custom properties belong to this device alone.
  public static let consumersPropertyName = "4cc_cnsm_glob_0000"
  public static let consumersSelector = fourCC("cnsm")

  /// What the extension's scheduled-output reports mean: `sent` when it reports a frame's number
  /// only after handing that frame to the meeting apps' stream. Without the property the helper
  /// trusts no report.
  public static let outputReceiptsPropertyName = "4cc_orcp_glob_0000"
  public static let outputReceiptsSelector = fourCC("orcp")
  public static let outputReceiptsValue = "sent"

  public static let width = 1280
  public static let height = 720
  public static let framesPerSecond: Int32 = 30

  /// A frame from the helper older than this no longer counts as live; the idle frame shows.
  public static let liveFrameMaxAge: TimeInterval = 0.5

  /// `kIOAudioDeviceTransportTypeVirtual`, which this extension and other virtual cameras report.
  public static let virtualTransportType = Int32(bitPattern: fourCC("virt"))

  /// Whether the helper may read a camera as the person's own: never this camera, and never any
  /// virtual camera. Two watermark cameras that each wrapped the other (Anarlog Camera and Char
  /// Camera) would feed each other's frames back instead of the person.
  public static func canWrap(deviceName: String, transportType: Int32) -> Bool {
    deviceName != Self.deviceName && transportType != virtualTransportType
  }
}

public func fourCC(_ code: String) -> UInt32 {
  code.utf8.reduce(0) { ($0 << 8) | UInt32($1) }
}

/// Who is using the camera, as the extension publishes it: one signing ID per line.
public enum ConsumerList {
  public static func encode(_ signingIDs: [String]) -> String {
    signingIDs.joined(separator: "\n")
  }

  public static func decode(_ value: String) -> [String] {
    value.split(separator: "\n").map(String.init).filter { !$0.isEmpty }
  }
}
