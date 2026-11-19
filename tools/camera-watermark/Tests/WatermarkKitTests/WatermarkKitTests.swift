import AVFoundation
import CoreGraphics
import CoreImage
import XCTest

@testable import WatermarkKit

final class FeedStateTests: XCTestCase {
  func testCameraStaysOffWhileNobodyUsesTheCamera() {
    let state = FeedState(recording: true, consumers: [])
    XCTAssertFalse(state.capturesCamera)
    XCTAssertFalse(state.showsBadge)
  }

  func testPlainCameraWhileNotRecording() {
    let state = FeedState(recording: false, consumers: ["us.zoom.xos"])
    XCTAssertTrue(state.capturesCamera)
    XCTAssertFalse(state.showsBadge)
  }

  func testBadgeWhileRecordingAndInUse() {
    XCTAssertTrue(FeedState(recording: true, consumers: ["us.zoom.xos"]).showsBadge)
  }
}

final class HelperProtocolTests: XCTestCase {
  func testParsesCommands() {
    XCTAssertEqual(HelperCommand.parse(#"{"command":"activate"}"#), .activate)
    XCTAssertEqual(HelperCommand.parse(#"{"command":"deactivate"}"#), .deactivate)
    XCTAssertEqual(HelperCommand.parse(#"{"command":"enable_agent"}"#), .enableAgent)
    XCTAssertEqual(HelperCommand.parse(#"{"command":"disable_agent"}"#), .disableAgent)
    XCTAssertEqual(HelperCommand.parse(#"{"command":"check"}"#), .check)
  }

  func testRejectsMalformedCommands() {
    XCTAssertNil(HelperCommand.parse("not json"))
    XCTAssertNil(HelperCommand.parse(#"{"command":"launch_rockets"}"#))
    XCTAssertNil(
      HelperCommand.parse(#"{"command":"set_recording","recording":true}"#),
      "what the camera shows goes to the agent's state file, not to serve")
    XCTAssertNil(
      HelperCommand.parse(#"{"command":"preview","id":"1","style":{}}"#),
      "Anarlog has no style previews")
  }

  func testEventLinesAreSortedJSON() {
    XCTAssertEqual(
      HelperEvent.line("extension", ["state": "active"]), #"{"event":"extension","state":"active"}"#
    )
  }

  func testConsumerListRoundTrips() {
    let ids = ["com.google.Chrome.helper", "us.zoom.xos"]
    XCTAssertEqual(ConsumerList.decode(ConsumerList.encode(ids)), ids)
    XCTAssertEqual(ConsumerList.decode(""), [])
  }

  func testCustomPropertySelectorMatchesItsName() {
    XCTAssertEqual(CameraIdentity.consumersSelector, 0x636E_736D)
    XCTAssertTrue(CameraIdentity.consumersPropertyName.hasPrefix("4cc_cnsm_"))
  }
}

final class SVGPathTests: XCTestCase {
  func testAbsoluteAndRelativeCommands() throws {
    let path = try SVGPath.parse("M0 0H10V10L0 10Zm20 0h5v5z")
    let box = path.boundingBoxOfPath
    XCTAssertEqual(box.minX, 0)
    XCTAssertEqual(box.maxX, 25)
    XCTAssertEqual(box.maxY, 10)
  }

  func testAnarlogWordmarkParsesToItsViewBox() throws {
    XCTAssertEqual(AnarlogBrand.word.count, 7, "one path per letter of anarlog")
    var union = CGRect.null
    for data in AnarlogBrand.word {
      let box = try SVGPath.parse(data).boundingBoxOfPath
      XCTAssertLessThanOrEqual(box.maxX, AnarlogBrand.logoSize.width + 0.5)
      XCTAssertLessThanOrEqual(box.maxY, AnarlogBrand.logoSize.height + 0.5)
      union = union.union(box)
    }
    XCTAssertEqual(union.minX, 0, accuracy: 1)
    XCTAssertEqual(union.maxX, AnarlogBrand.logoSize.width, accuracy: 1)
  }

  func testRejectsCommandsTheLogoDoesNotUse() {
    XCTAssertThrowsError(try SVGPath.parse("M0 0A1 1 0 0 1 2 2"))
  }
}

final class WatermarkStyleTests: XCTestCase {
  func testClampsWhatSettingsCouldGetWrong() throws {
    let style = try XCTUnwrap(
      WatermarkStyle.decode([
        "layout": "banner", "position": "top_right",
        "text": "  Anarlog\n is   taking notes for them all, the whole team, today  ",
        "color": "red", "opacity": 0.05, "image": "data:image/jpeg;base64,AAAA",
      ]))
    XCTAssertEqual(style.layout, .banner)
    XCTAssertEqual(style.position, .topRight)
    XCTAssertEqual(style.text, "Anarlog is taking notes for them")
    XCTAssertEqual(style.text.count, WatermarkStyle.maxTextLength)
    XCTAssertEqual(style.color, "#F04545", "a bad colour falls back to the layout's")
    XCTAssertEqual(style.opacity, 0.3)
    XCTAssertNil(style.image, "only PNG data URLs are drawn")
  }

  func testFallsBackFieldByField() throws {
    let style = try XCTUnwrap(
      WatermarkStyle.decode(["layout": "hologram", "text": "   ", "opacity": 7]))
    XCTAssertEqual(style, .badge)
    XCTAssertNil(WatermarkStyle.decode("badge"))
    XCTAssertNil(WatermarkStyle.decode(nil))
  }

  func testReadsThePersonsImage() throws {
    let ctx = try XCTUnwrap(
      CGContext(
        data: nil, width: 40, height: 20, bitsPerComponent: 8, bytesPerRow: 0,
        space: CGColorSpace(name: CGColorSpace.sRGB)!,
        bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
    ctx.setFillColor(CGColor(srgbRed: 1, green: 0, blue: 0, alpha: 1))
    ctx.fill(CGRect(x: 0, y: 0, width: 40, height: 20))
    let png = try XCTUnwrap(
      Compositor().png(XCTUnwrap(pixelBuffer(from: XCTUnwrap(ctx.makeImage())))))
    let style = try XCTUnwrap(
      WatermarkStyle.decode(["image": "data:image/png;base64,\(png.base64EncodedString())"]))
    XCTAssertEqual(style.image?.width, 40)
    XCTAssertEqual(style.image?.height, 20)
  }

  private func pixelBuffer(from image: CGImage) -> CVPixelBuffer? {
    guard let buffer = Watermark.makeBuffer(width: image.width, height: image.height) else {
      return nil
    }
    Watermark.withContext(on: buffer) { ctx, size in
      ctx.draw(image, in: CGRect(origin: .zero, size: size))
    }
    return buffer
  }
}

final class WatermarkLayoutTests: XCTestCase {
  /// Where a face sits in a webcam frame (y up): the middle, from the chin to the brow. A style
  /// along the top edge may cross the top of the head, not the face.
  private func faceBox(_ frame: CGSize) -> CGRect {
    CGRect(
      x: frame.width * 0.3, y: frame.height * 0.2, width: frame.width * 0.4,
      height: frame.height * 0.6)
  }

  private let sizes = [CGSize(width: 1280, height: 720), CGSize(width: 640, height: 360)]

  func testEveryStyleStaysInTheFrameAndOffTheFace() {
    for size in sizes {
      for layout in WatermarkStyle.Layout.allCases {
        for position in WatermarkStyle.Position.allCases {
          let style = WatermarkStyle(
            layout: layout, position: position, text: String(repeating: "W", count: 40),
            color: "#FFFFFF")
          let placed = WatermarkLayout.make(style, frame: size)
          let frame = CGRect(origin: .zero, size: size)
          for rect in placed.covered {
            XCTAssertTrue(frame.contains(rect), "\(layout) \(position) leaves the frame: \(rect)")
            XCTAssertFalse(
              rect.intersects(faceBox(size)), "\(layout) \(position) covers the face: \(rect)")
          }
        }
      }
    }
  }

  func testLongTextIsCutRatherThanWidened() {
    let size = CGSize(width: 1280, height: 720)
    let style = WatermarkStyle(
      layout: .badge, text: String(repeating: "W", count: 32), color: "#F04545")
    let plate = WatermarkLayout.make(style, frame: size).plate!
    XCTAssertLessThanOrEqual(plate.width, size.width * WatermarkLayout.maxPlateWidth + 1)
  }

  func testCornersAndEdges() {
    let size = CGSize(width: 1280, height: 720)
    let topRight = WatermarkLayout.make(
      WatermarkStyle(layout: .badge, position: .topRight, color: "#F04545"), frame: size
    ).plate!
    XCTAssertEqual(topRight.maxX, size.width - 29, accuracy: 1)
    XCTAssertEqual(topRight.maxY, size.height - 29, accuracy: 1)

    let banner = WatermarkLayout.make(.banner, frame: size).plate!
    XCTAssertEqual(banner, CGRect(x: 0, y: 0, width: 1280, height: 86))
    let topBanner = WatermarkLayout.make(
      WatermarkStyle(layout: .banner, position: .topRight, color: "#F04545"), frame: size)
    XCTAssertEqual(topBanner.plate!.maxY, size.height)
    XCTAssertGreaterThan(topBanner.logo.minX, size.width / 2, "its content keeps to the right")

    let mark = WatermarkLayout.make(.mark, frame: size)
    XCTAssertNil(mark.plate)
    XCTAssertNil(mark.text)
    XCTAssertEqual(mark.logo.maxX, size.width - 29, accuracy: 1)
  }

  func testTheImageTakesTheLogosPlaceWithinItsBounds() throws {
    let ctx = try XCTUnwrap(
      CGContext(
        data: nil, width: 1000, height: 100, bitsPerComponent: 8, bytesPerRow: 0,
        space: CGColorSpace(name: CGColorSpace.sRGB)!,
        bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
    let wide = WatermarkStyle(layout: .badge, color: "#F04545", image: ctx.makeImage())
    let layout = WatermarkLayout.make(wide, frame: CGSize(width: 1280, height: 720))
    let plate = layout.plate!
    XCTAssertLessThanOrEqual(
      layout.logo.width, plate.height * 3 + 0.5, "a very wide logo is capped")
    XCTAssertEqual(
      layout.logo.width / layout.logo.height, 10, accuracy: 0.01, "and keeps its shape")
  }
}

final class WatermarkTests: XCTestCase {
  func testBadgeSitsInTheBottomLeftAndLeavesTheRestAlone() throws {
    let white = CIImage(color: CIColor(red: 1, green: 1, blue: 1)).cropped(
      to: CGRect(x: 0, y: 0, width: 1920, height: 1080))
    let frame = try XCTUnwrap(Compositor().compose(white, overlay: .badge))
    XCTAssertEqual(CVPixelBufferGetWidth(frame), CameraIdentity.width)
    XCTAssertEqual(CVPixelBufferGetHeight(frame), CameraIdentity.height)
    // Memory rows run top to bottom: the badge is near the last rows.
    let pill = pixel(frame, x: 60, y: CameraIdentity.height - 40)
    XCTAssertLessThan(pill.r, 120, "the pill darkens the bottom-left corner")
    let topRight = pixel(frame, x: CameraIdentity.width - 20, y: 20)
    XCTAssertGreaterThan(topRight.r, 240, "the rest of the frame is the camera's")
  }

  func testNoBadgeLeavesTheFrameUntouched() throws {
    let white = CIImage(color: CIColor(red: 1, green: 1, blue: 1)).cropped(
      to: CGRect(x: 0, y: 0, width: 1280, height: 720))
    let frame = try XCTUnwrap(Compositor().compose(white, overlay: nil))
    XCTAssertGreaterThan(pixel(frame, x: 60, y: CameraIdentity.height - 40).r, 240)
  }

  /// Each preset draws where its layout says and leaves the middle of the frame alone.
  func testEachPresetDrawsWhereItsLayoutSays() throws {
    let gray = CIImage(color: CIColor(red: 0.5, green: 0.5, blue: 0.5)).cropped(
      to: CGRect(x: 0, y: 0, width: 1280, height: 720))
    let size = CGSize(width: CameraIdentity.width, height: CameraIdentity.height)
    for style in [WatermarkStyle.badge, .glow, .mark, .banner] {
      let frame = try XCTUnwrap(Compositor().compose(gray, overlay: style))
      let middle = pixel(frame, x: CameraIdentity.width / 2, y: CameraIdentity.height / 2)
      XCTAssertEqual(Int(middle.r), 128, accuracy: 2, "\(style.layout) leaves the face alone")
      // The plate's left padding, or the mark's ink, in memory rows (top to bottom).
      let layout = WatermarkLayout.make(style, frame: size)
      let probe =
        layout.plate.map { CGPoint(x: $0.minX + $0.height * 0.2, y: $0.midY) }
        ?? CGPoint(x: layout.logo.minX + 2, y: layout.logo.midY)
      // How far a pixel is from the gray it was drawn over, summed over its channels.
      func change(_ x: Int) -> Int {
        let p = pixel(frame, x: x, y: CameraIdentity.height - Int(probe.y))
        return abs(Int(p.r) - 128) + abs(Int(p.g) - 128) + abs(Int(p.b) - 128)
      }
      let drawn =
        style.layout == .mark
        ? (Int(layout.logo.minX)..<Int(layout.logo.maxX)).map(change).max() ?? 0
        : change(Int(probe.x))
      XCTAssertGreaterThan(drawn, 120, "\(style.layout) draws at \(probe)")
      if style.layout == .glow {
        XCTAssertGreaterThan(Int(pixel(frame, x: 1, y: 300).b), 180, "the glow tints the edge")
      }
    }
  }

  func testIdleFrameIsDarkWithTheWordmarkInTheMiddle() throws {
    let frame = try XCTUnwrap(Watermark.makeBuffer())
    Watermark.withContext(on: frame) { Watermark.drawIdle(in: $0, frame: $1) }
    XCTAssertLessThan(pixel(frame, x: 10, y: 10).r, 40)
    let size = CGSize(width: CameraIdentity.width, height: CameraIdentity.height)
    let logo = Watermark.idleLogoRect(frame: size)
    XCTAssertEqual(logo.midX, size.width / 2, accuracy: 0.5)
    // Across the letters' middle (y up, so memory rows count from the top) there is white ink.
    let row = CameraIdentity.height - Int(logo.minY + logo.height * 0.4)
    let brightest =
      (Int(logo.minX)...Int(logo.maxX)).map { pixel(frame, x: $0, y: row).r }.max() ?? 0
    XCTAssertGreaterThan(brightest, 200)
  }

  private func pixel(_ buffer: CVPixelBuffer, x: Int, y: Int) -> (r: UInt8, g: UInt8, b: UInt8) {
    CVPixelBufferLockBaseAddress(buffer, .readOnly)
    defer { CVPixelBufferUnlockBaseAddress(buffer, .readOnly) }
    let base = CVPixelBufferGetBaseAddress(buffer)!.assumingMemoryBound(to: UInt8.self)
    let offset = y * CVPixelBufferGetBytesPerRow(buffer) + x * 4
    return (base[offset + 2], base[offset + 1], base[offset])
  }
}

final class WatermarkMotionTests: XCTestCase {
  private let loop = WatermarkMotion.loop

  /// Every moving part fits the loop a whole number of times: the dot breathes three times, the
  /// glow twice, the Mark sweeps once.
  func testEveryPeriodFitsTheLoop() {
    XCTAssertEqual(loop, 112.0 / 24, accuracy: 1e-12)
    XCTAssertEqual(loop / WatermarkMotion.dotPeriod, 3, accuracy: 1e-12)
    XCTAssertEqual(loop / WatermarkMotion.glowPeriod, 2, accuracy: 1e-12)
  }

  func testTheDotBreathesThreeTimesALoop() {
    XCTAssertEqual(
      WatermarkMotion.dotAlpha(at: 0), 1, accuracy: 1e-9, "the still frame shows it lit")
    // Faintest halfway through each breath (about 0.78 s, 2.33 s, 3.89 s), lit again between them.
    for breath in 0..<3 {
      let start = Double(breath) * loop / 3
      XCTAssertEqual(WatermarkMotion.dotAlpha(at: start), 1, accuracy: 1e-9)
      XCTAssertEqual(
        WatermarkMotion.dotAlpha(at: start + loop / 6), WatermarkMotion.dotMinAlpha, accuracy: 1e-9)
    }
    XCTAssertEqual(
      WatermarkMotion.dotAlpha(at: 0.389), 0.675, accuracy: 0.01,
      "a quarter breath in, halfway down")
    XCTAssertGreaterThanOrEqual(
      WatermarkMotion.dotPeriod, 1.5, "slower than one breath every 1.5 s")
  }

  func testTheMarkSweepsToTheAccentAndBackOnceALoop() {
    for u in [0, 0.5, 1.0] {
      XCTAssertEqual(
        WatermarkMotion.markAccent(at: 0, along: u), 0, accuracy: 1e-9, "own colour at rest")
      XCTAssertEqual(
        WatermarkMotion.markAccent(at: loop / 2, along: u), 1, accuracy: 1e-9, "accent halfway")
      XCTAssertEqual(
        WatermarkMotion.markAccent(at: loop * 0.97, along: u), 0, accuracy: 1e-9, "own colour again"
      )
    }
    // A quarter in, the front is mid-sweep: further along it hasn't arrived yet.
    let quarter = loop * 0.25
    XCTAssertGreaterThan(
      WatermarkMotion.markAccent(at: quarter, along: 0),
      WatermarkMotion.markAccent(at: quarter, along: 1))
  }

  func testTheGlowBreathesTwiceALoop() {
    XCTAssertEqual(WatermarkMotion.glowStrength(at: 0), 1, accuracy: 1e-9)
    XCTAssertEqual(
      WatermarkMotion.glowStrength(at: loop / 4), WatermarkMotion.glowMinStrength, accuracy: 1e-9)
    XCTAssertEqual(WatermarkMotion.glowStrength(at: loop / 2), 1, accuracy: 1e-9)
    XCTAssertEqual(
      WatermarkMotion.glowStrength(at: loop * 3 / 4), WatermarkMotion.glowMinStrength,
      accuracy: 1e-9)
    XCTAssertEqual(
      WatermarkMotion.imageAlpha(at: loop / 4), WatermarkMotion.imageMinAlpha, accuracy: 1e-9)
  }

  /// Every moving part is periodic, so a loop played on repeat (or a camera running for hours)
  /// never jumps: the end of one loop meets the start of the next.
  func testEveryLoopMeetsItsStart() {
    let parts: [(String, (Double) -> Double)] = [
      ("dot", WatermarkMotion.dotAlpha), ("glow", WatermarkMotion.glowStrength),
      ("image", WatermarkMotion.imageAlpha),
      ("mark left", { WatermarkMotion.markAccent(at: $0, along: 0) }),
      ("mark right", { WatermarkMotion.markAccent(at: $0, along: 1) }),
    ]
    for (name, f) in parts {
      XCTAssertEqual(f(loop - 1e-6), f(0), accuracy: 1e-4, "\(name) jumps at the wrap")
      XCTAssertEqual(f(loop * 1000 + 0.3), f(0.3), accuracy: 1e-6, "\(name) drifts over long runs")
      XCTAssertEqual(f(-0.3), f(loop - 0.3), accuracy: 1e-9, "\(name) at negative times")
    }
  }

  /// Gentle, not strobing: from one 30 fps frame to the next nothing changes by more than a few
  /// percent.
  func testNothingJumpsBetweenFrames() {
    let step = 1.0 / 30
    for i in 0..<Int(loop * 30) {
      let t = Double(i) * step
      XCTAssertLessThan(
        abs(WatermarkMotion.dotAlpha(at: t + step) - WatermarkMotion.dotAlpha(at: t)), 0.06)
      XCTAssertLessThan(
        abs(WatermarkMotion.glowStrength(at: t + step) - WatermarkMotion.glowStrength(at: t)), 0.06)
      for u in stride(from: 0.0, through: 1, by: 0.25) {
        XCTAssertLessThan(
          abs(
            WatermarkMotion.markAccent(at: t + step, along: u)
              - WatermarkMotion.markAccent(at: t, along: u)),
          0.15)
      }
    }
  }

  func testTheStillFrameIsTimeZero() throws {
    let gray = CIImage(color: CIColor(red: 0.5, green: 0.5, blue: 0.5)).cropped(
      to: CGRect(x: 0, y: 0, width: 1280, height: 720))
    let compositor = Compositor()
    for style in [WatermarkStyle.badge, .glow, .mark, .banner] {
      let still = try XCTUnwrap(compositor.compose(gray, overlay: style))
      let atZero = try XCTUnwrap(compositor.compose(gray, overlay: style, time: 0))
      let later = try XCTUnwrap(
        compositor.compose(gray, overlay: style, time: WatermarkMotion.loop / 4))
      XCTAssertEqual(bytes(still), bytes(atZero), "\(style.layout)")
      XCTAssertNotEqual(bytes(still), bytes(later), "\(style.layout) moves")
    }
  }

  private func bytes(_ buffer: CVPixelBuffer) -> Data {
    CVPixelBufferLockBaseAddress(buffer, .readOnly)
    defer { CVPixelBufferUnlockBaseAddress(buffer, .readOnly) }
    return Data(
      bytes: CVPixelBufferGetBaseAddress(buffer)!,
      count: CVPixelBufferGetBytesPerRow(buffer) * CVPixelBufferGetHeight(buffer))
  }
}

final class SinkGateTests: XCTestCase {
  private let helper = UUID()
  private let other = UUID()
  private let team = "AAAAAAAAAA"

  func testOnlyTheHelperSignedByTheExtensionsTeamMayWrite() {
    var gate = SinkGate(teamID: team)
    XCTAssertEqual(
      gate.admit(client: other, signingID: "com.example.cam", clientTeamID: team), .notHelper)
    XCTAssertEqual(gate.admit(client: other, signingID: nil, clientTeamID: nil), .notHelper)
    XCTAssertEqual(
      gate.admit(client: other, signingID: SinkGate.helperSigningID, clientTeamID: nil),
      .notOurTeam,
      "a process that only claims the helper's identifier is refused")
    XCTAssertEqual(
      gate.admit(client: other, signingID: SinkGate.helperSigningID, clientTeamID: "BBBBBBBBBB"),
      .notOurTeam, "another team's build of the helper is refused")
    XCTAssertEqual(
      gate.admit(client: other, signingID: "com.char.desktop.camera", clientTeamID: team),
      .notHelper,
      "Char Camera's helper never writes into Anarlog Camera")
    XCTAssertNil(gate.writer)
    XCTAssertEqual(
      gate.admit(client: helper, signingID: SinkGate.helperSigningID, clientTeamID: team), .accept)
  }

  func testAnExtensionWithoutATeamAdmitsNobody() {
    for unsigned in [nil, ""] as [String?] {
      var gate = SinkGate(teamID: unsigned)
      XCTAssertNil(gate.teamID)
      XCTAssertEqual(
        gate.admit(client: helper, signingID: SinkGate.helperSigningID, clientTeamID: nil),
        .notOurTeam)
      XCTAssertEqual(
        gate.admit(client: helper, signingID: SinkGate.helperSigningID, clientTeamID: ""),
        .notOurTeam)
    }
  }

  func testOneWriterAtATimeUntilItStopsOrGoesAway() {
    var gate = SinkGate(teamID: team)
    let id = SinkGate.helperSigningID
    XCTAssertEqual(gate.admit(client: helper, signingID: id, clientTeamID: team), .accept)
    XCTAssertEqual(gate.admit(client: other, signingID: id, clientTeamID: team), .busy)
    XCTAssertEqual(
      gate.admit(client: helper, signingID: id, clientTeamID: team), .accept,
      "the writer may restart")
    gate.release(other)
    XCTAssertEqual(gate.writer, helper, "another client going away leaves the writer")
    gate.release(helper)
    XCTAssertEqual(gate.admit(client: other, signingID: id, clientTeamID: team), .accept)
    gate.release(nil)
    XCTAssertNil(gate.writer, "a stopped stream frees the sink")
  }

  func testThisTestRunnerIsNotTheHelper() {
    XCTAssertNil(SignedClient.verifiedTeam(pid: getpid(), expected: team))
    XCTAssertNil(SignedClient.verifiedTeam(pid: getpid(), expected: nil))
  }

  func testReadsAnotherProcessSigningIDFromTheKernel() throws {
    // Finder is always running and Apple-signed; CMIO's own field would say "unknown".
    let running = NSRunningApplication.runningApplications(withBundleIdentifier: "com.apple.finder")
    let finder = try XCTUnwrap(running.first ?? { throw XCTSkip("no Finder on this runner") }())
    XCTAssertEqual(SignedClient.signingID(pid: finder.processIdentifier), "com.apple.finder")
    XCTAssertNil(SignedClient.verifiedTeam(pid: finder.processIdentifier, expected: team))
  }
}

final class WatermarkBackgroundTests: XCTestCase {
  func testDecodesEachKind() {
    XCTAssertEqual(WatermarkBackground.decode(["kind": "none"]), WatermarkBackground.none)
    XCTAssertEqual(WatermarkBackground.decode(["kind": "blur"]), .blur)
    XCTAssertEqual(WatermarkBackground.decode(["kind": "image", "image": "dusk"]), .scene(.dusk))
    XCTAssertNil(WatermarkBackground.decode("blur"), "only an object is a background")
  }

  /// Anarlog waits for `blur` or `image` before turning a meeting app's own effect off.
  func testReportsItsKindForTheAgentStatus() {
    XCTAssertEqual(WatermarkBackground.none.kind, "none")
    XCTAssertEqual(WatermarkBackground.blur.kind, "blur")
    XCTAssertEqual(WatermarkBackground.scene(.dusk).kind, "image")
    XCTAssertEqual(WatermarkBackground.decode(["kind": "image", "image": "custom"])?.kind, "image")
  }

  /// A person who asked to hide their room never sees it come back over a bad value.
  func testABadImageStillHidesTheRoom() {
    XCTAssertEqual(WatermarkBackground.decode(["kind": "image", "image": "moon"]), .scene(.studio))
    XCTAssertEqual(
      WatermarkBackground.decode([
        "kind": "image", "image": "custom", "custom": "data:image/gif;base64,AAAA",
      ]),
      .scene(.studio))
    XCTAssertEqual(
      WatermarkBackground.decode(["kind": "image", "image": "custom"]), .scene(.studio))
    XCTAssertEqual(WatermarkBackground.decode(["kind": "hologram"]), WatermarkBackground.none)
  }

  func testReadsThePersonsOwnImage() throws {
    let image = try XCTUnwrap(solidImage(width: 64, height: 36, red: 0, green: 0, blue: 1))
    let data = try XCTUnwrap(jpeg(image))
    let decoded = WatermarkBackground.decode([
      "kind": "image", "image": "custom",
      "custom": "data:image/jpeg;base64,\(data.base64EncodedString())",
    ])
    guard case .image(let own) = decoded else { return XCTFail("expected the person's image") }
    XCTAssertEqual(own.width, 64)
    XCTAssertEqual(
      WatermarkBackground.decode([
        "kind": "image", "image": "studio",
        "custom": "data:image/jpeg;base64,\(data.base64EncodedString())",
      ]), .scene(.studio), "a scene chosen over a kept image draws the scene")
  }

  /// Nobody in the frame: everything is the background, and the badge is drawn on top of it.
  func testTheSceneFillsTheRoomAndTheBadgeStaysOnTop() throws {
    let red = CIImage(color: CIColor(red: 1, green: 0, blue: 0)).cropped(
      to: CGRect(x: 0, y: 0, width: 1280, height: 720))
    let scene = try XCTUnwrap(Compositor().compose(red, overlay: nil, background: .scene(.studio)))
    let badged = try XCTUnwrap(
      Compositor().compose(red, overlay: .badge, background: .scene(.studio)))
    let corner = pixel(scene, x: CameraIdentity.width - 20, y: 20)
    XCTAssertLessThan(
      Int(corner.r) - Int(corner.b), 40, "the red room is gone: Studio's grey is there")
    // The pill's left padding and the dot, in memory rows (top to bottom).
    let layout = WatermarkLayout.make(
      .badge, frame: CGSize(width: CameraIdentity.width, height: CameraIdentity.height))
    let plate = try XCTUnwrap(layout.plate)
    let dot = try XCTUnwrap(layout.dot)
    let pad = (x: Int(plate.minX + plate.height * 0.2), y: CameraIdentity.height - Int(plate.midY))
    let under = pixel(scene, x: pad.x, y: pad.y)
    let over = pixel(badged, x: pad.x, y: pad.y)
    XCTAssertGreaterThan(Int(under.g) - Int(over.g), 50, "the pill darkens the scene under it")
    let red2 = pixel(badged, x: Int(dot.midX), y: CameraIdentity.height - Int(dot.midY))
    XCTAssertGreaterThan(Int(red2.r) - Int(red2.g), 120, "the red dot shows over the scene")
  }

  func testBlurSoftensTheRoom() throws {
    // A hard edge down the middle of an empty room.
    let half = CGRect(x: 0, y: 0, width: 640, height: 720)
    let room = CIImage(color: CIColor(red: 1, green: 1, blue: 1)).cropped(to: half)
      .composited(
        over: CIImage(color: CIColor(red: 0, green: 0, blue: 0)).cropped(
          to: CGRect(x: 0, y: 0, width: 1280, height: 720)))
    let sharp = try XCTUnwrap(Compositor().compose(room, overlay: nil))
    let blurred = try XCTUnwrap(Compositor().compose(room, overlay: nil, background: .blur))
    XCTAssertGreaterThan(pixel(sharp, x: 636, y: 360).r, 240)
    let edge = pixel(blurred, x: 636, y: 360).r
    XCTAssertTrue((60...200).contains(edge), "the edge is soft: \(edge)")
    XCTAssertGreaterThan(
      pixel(blurred, x: 100, y: 360).r, 240, "far from the edge, the room's own colour")
  }

  private func solidImage(width: Int, height: Int, red: CGFloat, green: CGFloat, blue: CGFloat)
    -> CGImage?
  {
    let ctx = CGContext(
      data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0,
      space: CGColorSpace(name: CGColorSpace.sRGB)!,
      bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
    ctx?.setFillColor(CGColor(srgbRed: red, green: green, blue: blue, alpha: 1))
    ctx?.fill(CGRect(x: 0, y: 0, width: width, height: height))
    return ctx?.makeImage()
  }

  private func jpeg(_ image: CGImage) -> Data? {
    let data = NSMutableData()
    guard let dest = CGImageDestinationCreateWithData(data, "public.jpeg" as CFString, 1, nil)
    else {
      return nil
    }
    CGImageDestinationAddImage(dest, image, nil)
    return CGImageDestinationFinalize(dest) ? data as Data : nil
  }

  private func pixel(_ buffer: CVPixelBuffer, x: Int, y: Int) -> (r: UInt8, g: UInt8, b: UInt8) {
    CVPixelBufferLockBaseAddress(buffer, .readOnly)
    defer { CVPixelBufferUnlockBaseAddress(buffer, .readOnly) }
    let base = CVPixelBufferGetBaseAddress(buffer)!.assumingMemoryBound(to: UInt8.self)
    let offset = y * CVPixelBufferGetBytesPerRow(buffer) + x * 4
    return (base[offset + 2], base[offset + 1], base[offset])
  }
}

final class AgentStateTests: XCTestCase {
  func testNoFileIsWatermarkOff() {
    XCTAssertEqual(AgentState.decode(nil), AgentState())
    XCTAssertEqual(AgentState.decode(Data("{not json".utf8)), AgentState())
    XCTAssertFalse(AgentState().enabled)
  }

  func testReadsAStyle() throws {
    let json =
      ##"{"enabled":true,"recording":true,"owner_pid":42,"style":{"layout":"glow","position":"bottom_right","color":"#5B8DEF"},"background":{"kind":"blur"},"background_id":"b-7","camera_request":3}"##
    let state = AgentState.decode(Data(json.utf8))
    XCTAssertEqual(
      state,
      AgentState(
        enabled: true, recording: true, ownerPID: 42, style: .glow, background: .blur,
        backgroundID: "b-7", cameraRequest: 3))
  }

  /// Exactly what `plugins/camera-watermark` (`Helper::write_state`) writes: no style, so the
  /// badge; a scene as `image`; the camera request in milliseconds.
  func testReadsWhatAnarlogWrites() throws {
    let json =
      ##"{"background":{"image":"dusk","kind":"image"},"background_id":"6f1c2c1e-6a0e-4c56-9f0e-3f7d2b1a9c00","camera_request":1763380000000,"enabled":true,"owner_pid":42,"recording":false}"##
    XCTAssertEqual(
      AgentState.decode(Data(json.utf8)),
      AgentState(
        enabled: true, recording: false, ownerPID: 42, style: .badge, background: .scene(.dusk),
        backgroundID: "6f1c2c1e-6a0e-4c56-9f0e-3f7d2b1a9c00", cameraRequest: 1_763_380_000_000))
  }

  /// The badge never claims a recording that isn't happening: Anarlog must still be running, and
  /// the camera on.
  func testRecordsOnlyWhileAnarlogLives() {
    let state = AgentState(enabled: true, recording: true, ownerPID: 42)
    XCTAssertTrue(state.records { $0 == 42 })
    XCTAssertFalse(state.records { _ in false }, "Anarlog quit or crashed mid-recording")
    XCTAssertFalse(AgentState(enabled: false, recording: true, ownerPID: 42).records { _ in true })
    XCTAssertFalse(AgentState(enabled: true, recording: true, ownerPID: 0).records { _ in true })
    XCTAssertTrue(
      AgentState(
        enabled: true, recording: true, ownerPID: Int32(ProcessInfo.processInfo.processIdentifier)
      )
      .records { kill($0, 0) == 0 }, "this process is alive")
  }
}

/// The receipt Anarlog needs before it turns a meeting app's own effect off: frames that hide
/// the room for the Background it asked for, sent by the extension, lately. The look the agent
/// wants is not enough.
final class ProtectedOutputTests: XCTestCase {
  private func output() -> ProtectedOutput { ProtectedOutput(firstSequence: 100) }

  func testWantingABackgroundIsNotSendingIt() {
    var output = output()
    XCTAssertFalse(output.hidesRoom(for: "blur-1", now: 0), "nothing drawn yet")
    output.enqueued(backgroundID: "blur-1", hidesRoom: true)
    XCTAssertFalse(
      output.hidesRoom(for: "blur-1", now: 0),
      "queued, but the extension hasn't sent it: nobody streams, or it's behind")
    output.sent(100, at: 0)
    XCTAssertTrue(output.hidesRoom(for: "blur-1", now: 0.5))
  }

  func testAFrameDrawnBeforeTheRequestDoesNotCount() {
    var output = output()
    // The room, still queued when Anarlog asked for blur.
    output.enqueued(backgroundID: "none-1", hidesRoom: false)
    output.enqueued(backgroundID: "blur-2", hidesRoom: true)
    output.sent(100, at: 1)
    XCTAssertFalse(output.hidesRoom(for: "blur-2", now: 1), "the old frame just went out")
    output.sent(101, at: 1.03)
    XCTAssertTrue(output.hidesRoom(for: "blur-2", now: 1.03))
  }

  func testAReceiptForAnEarlierRequestNeverCountsForANewerOne() {
    var output = output()
    output.enqueued(backgroundID: "blur-1", hidesRoom: true)
    output.sent(100, at: 0)
    // Blur again, but asked anew (the person went to none and back): the new request needs its
    // own frame out.
    XCTAssertFalse(output.hidesRoom(for: "blur-3", now: 0.1))
    XCTAssertFalse(output.hidesRoom(for: "", now: 0.1), "a state that names no request")
  }

  func testAStalledOutputProvesNothing() {
    var output = output()
    output.enqueued(backgroundID: "blur-1", hidesRoom: true)
    output.sent(100, at: 0)
    XCTAssertFalse(output.hidesRoom(for: "blur-1", now: ProtectedOutput.maxAge + 0.1))
  }

  func testReportsItDidNotNumberAreIgnored() {
    var output = output()
    output.enqueued(backgroundID: "blur-1", hidesRoom: true)
    // An extension that numbers frames itself, or reports a frame from before the sink restarted.
    output.sent(0, at: 0)
    output.sent(99, at: 0)
    XCTAssertFalse(output.hidesRoom(for: "blur-1", now: 0))
    // Reports out of order can't bring back an older frame.
    output.enqueued(backgroundID: "blur-1", hidesRoom: true)
    output.sent(101, at: 0)
    output.sent(100, at: 0.01)
    XCTAssertTrue(output.hidesRoom(for: "blur-1", now: 0.01))
  }

  func testAStoppedSinkForgetsWhatWentOut() {
    var output = output()
    output.enqueued(backgroundID: "blur-1", hidesRoom: true)
    output.sent(100, at: 0)
    output.enqueued(backgroundID: "blur-1", hidesRoom: true)
    output.reset()
    XCTAssertFalse(output.hidesRoom(for: "blur-1", now: 0))
    output.sent(101, at: 0)
    XCTAssertFalse(output.hidesRoom(for: "blur-1", now: 0), "queued before the stop")
  }

  func testANoneBackgroundNeverHidesTheRoom() {
    var output = output()
    output.enqueued(backgroundID: "none-1", hidesRoom: false)
    output.sent(100, at: 0)
    XCTAssertFalse(output.hidesRoom(for: "none-1", now: 0))
  }

  func testFreshNumbersNeverStartWhereAnotherNumberingWould() {
    XCTAssertGreaterThanOrEqual(ProtectedOutput().nextSequence, 1 << 40)
  }
}

/// Anarlog Camera installs, registers and runs beside Char Camera: no name, ID, file or label
/// collides with Char's.
final class IdentityTests: XCTestCase {
  func testNothingCollidesWithCharCamera() {
    let ours = [
      CameraIdentity.deviceName, CameraIdentity.helperBundleID, CameraIdentity.extensionBundleID,
      AgentFiles.label, AgentFiles.directory.lastPathComponent,
    ]
    let chars = [
      "Char Camera", "Char Watermark", "com.char.desktop.camera",
      "com.char.desktop.camera.extension",
      "com.char.desktop.camera.agent",
    ]
    for value in ours { XCTAssertFalse(chars.contains(value), value) }
    XCTAssertEqual(AgentFiles.directory.lastPathComponent, CameraIdentity.helperBundleID)
    XCTAssertEqual(AgentFiles.label, "\(CameraIdentity.helperBundleID).agent")
    XCTAssertTrue(CameraIdentity.extensionBundleID.hasPrefix("\(CameraIdentity.helperBundleID)."))
    let charDevice = UUID(uuidString: "2E9EB91D-161C-43F8-A855-4272964E6B00")!
    XCTAssertNotEqual(CameraIdentity.deviceUUID, charDevice)
    XCTAssertEqual(
      Set([
        CameraIdentity.deviceUUID, CameraIdentity.sourceStreamUUID, CameraIdentity.sinkStreamUUID,
      ]).count, 3)
  }

  func testNeverWrapsAVirtualCamera() {
    XCTAssertTrue(CameraIdentity.canWrap(deviceName: "MacBook Pro Camera", transportType: 0))
    XCTAssertTrue(
      CameraIdentity.canWrap(
        deviceName: "Studio Display Camera", transportType: Int32(bitPattern: fourCC("usb "))))
    XCTAssertFalse(CameraIdentity.canWrap(deviceName: CameraIdentity.deviceName, transportType: 0))
    XCTAssertFalse(
      CameraIdentity.canWrap(
        deviceName: "Char Camera", transportType: CameraIdentity.virtualTransportType),
      "two watermark cameras must never wrap each other")
  }
}
