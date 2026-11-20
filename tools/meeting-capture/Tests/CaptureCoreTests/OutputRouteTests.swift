import CoreAudio
import XCTest

@testable import CaptureCore

final class OutputRouteTests: XCTestCase {
  private func route(
    _ name: String, _ transport: UInt32, uid: String = "", dataSource: UInt32? = nil
  ) -> OutputRoute {
    OutputRoute(name: name, uid: uid, transport: transport, dataSource: dataSource)
  }

  func testBuiltInOutputIsHeadphonesOnlyOnTheHeadphoneJack() {
    let builtIn = kAudioDeviceTransportTypeBuiltIn
    let speakers: UInt32 = 0x6973_706B  // 'ispk'
    XCTAssertFalse(
      route("MacBook Pro Speakers", builtIn, uid: "BuiltInSpeakerDevice", dataSource: speakers)
        .isHeadphones)
    // Intel Macs switch the built-in output's data source when headphones are plugged in.
    XCTAssertTrue(
      route("Built-in Output", builtIn, dataSource: OutputRoute.headphoneDataSource).isHeadphones)
    // Apple silicon gives the jack its own device; its UID still says so in a localized macOS.
    XCTAssertTrue(
      route("외장 헤드폰", builtIn, uid: "BuiltInHeadphoneOutputDevice").isHeadphones)
    XCTAssertTrue(route("External Headphones", builtIn).isHeadphones)
  }

  func testBluetoothAndUSBHeadphonesAreHeadphones() {
    let bluetooth = kAudioDeviceTransportTypeBluetooth
    for name in [
      "John’s AirPods Pro", "AirPods Max", "Beats Studio Buds", "Powerbeats Pro",
      "Bose QC Ultra Headphones", "Bose QuietComfort 45", "WH-1000XM5", "Sony WF-1000XM4",
      "Galaxy Buds2 Pro", "Pixel Buds", "Jabra Evolve2 Headset", "Shokz OpenRun earphones",
    ] {
      XCTAssertTrue(route(name, bluetooth).isHeadphones, name)
    }
    XCTAssertTrue(
      route("AirPods Pro", kAudioDeviceTransportTypeBluetoothLE).isHeadphones)
    XCTAssertTrue(
      route("Logitech G Pro X Headset", kAudioDeviceTransportTypeUSB)
        .isHeadphones)
  }

  // A speaker taken for headphones would put the call's echo on the microphone.
  func testSpeakersStayOffTheHeadphoneRoute() {
    let bluetooth = kAudioDeviceTransportTypeBluetooth
    for name in [
      "Beats Pill", "Bose SoundLink Flex", "Jabra Speak 750", "Anker PowerConf Speakerphone",
      "JBL Flip 6", "HomePod", "Sonos Roam", "Living Room", "Bose Smart Soundbar",
    ] {
      XCTAssertFalse(route(name, bluetooth).isHeadphones, name)
    }
    XCTAssertFalse(route("Logitech Webcam C925e", kAudioDeviceTransportTypeUSB).isHeadphones)
  }

  func testOtherTransportsStayOffTheHeadphoneRoute() {
    for transport in [
      kAudioDeviceTransportTypeHDMI, kAudioDeviceTransportTypeDisplayPort,
      kAudioDeviceTransportTypeAirPlay, kAudioDeviceTransportTypeVirtual,
      kAudioDeviceTransportTypeAggregate, kAudioDeviceTransportTypeUnknown,
    ] {
      XCTAssertFalse(route("AirPods Pro Headphones", transport).isHeadphones)
    }
  }

  func testOnlyHeadphonesTurnVoiceProcessingOff() {
    XCTAssertFalse(
      microphoneUsesVoiceProcessing(on: route("AirPods Pro", kAudioDeviceTransportTypeBluetooth)))
    XCTAssertTrue(
      microphoneUsesVoiceProcessing(
        on: route("MacBook Pro Speakers", kAudioDeviceTransportTypeBuiltIn)))
    XCTAssertTrue(
      microphoneUsesVoiceProcessing(on: route("LC27RG50", kAudioDeviceTransportTypeDisplayPort)))
    // No default output, or one Core Audio can't describe: keep echo cancellation.
    XCTAssertTrue(microphoneUsesVoiceProcessing(on: nil))
  }

  /// A trial mode holds on every route, so an output switch never flips it, and only `automatic`
  /// follows the route.
  func testTrialProcessingModesIgnoreTheRoute() {
    let headphones = route("AirPods Pro", kAudioDeviceTransportTypeBluetooth)
    let speakers = route("MacBook Pro Speakers", kAudioDeviceTransportTypeBuiltIn)
    for output in [headphones, speakers, nil] {
      XCTAssertTrue(microphoneUsesVoiceProcessing(on: output, processing: .voice))
      XCTAssertTrue(microphoneUsesVoiceProcessing(on: output, processing: .voiceAdvancedDucking))
      XCTAssertFalse(microphoneUsesVoiceProcessing(on: output, processing: .plain))
    }
    XCTAssertFalse(microphoneUsesVoiceProcessing(on: headphones, processing: .automatic))
    XCTAssertTrue(microphoneUsesVoiceProcessing(on: speakers, processing: .automatic))
    XCTAssertEqual(
      MicrophoneProcessing.allCases.filter(\.advancedDucking), [.voiceAdvancedDucking])
    XCTAssertEqual(MicrophoneProcessing(rawValue: "voice-advanced-ducking"), .voiceAdvancedDucking)
  }
}
