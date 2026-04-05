import Foundation

// MARK: - Daemon → UI (stdin)

enum InboundMessage {
  case state(StateMessage)
  case levels(LevelsMessage)
  case dismiss
}

struct StateMessage: Decodable {
  let recording: Bool?
  let status: String?
}

struct LevelsMessage: Decodable {
  let left: Float
  let right: Float
}

extension InboundMessage {
  static func parse(_ line: String) -> InboundMessage? {
    guard let data = line.data(using: .utf8) else { return nil }

    struct Envelope: Decodable {
      let type: String
    }

    guard let envelope = try? JSONDecoder().decode(Envelope.self, from: data) else {
      return nil
    }

    switch envelope.type {
    case "state":
      guard let msg = try? JSONDecoder().decode(StateMessage.self, from: data) else {
        return nil
      }
      return .state(msg)
    case "levels":
      guard let msg = try? JSONDecoder().decode(LevelsMessage.self, from: data) else {
        return nil
      }
      return .levels(msg)
    case "dismiss":
      return .dismiss
    default:
      return nil
    }
  }
}

// MARK: - UI → Daemon (stdout)

struct OutboundAction: Encodable {
  let type = "action"
  let action: String
}

func sendAction(_ action: String) {
  let msg = OutboundAction(action: action)
  guard let data = try? JSONEncoder().encode(msg),
    let json = String(data: data, encoding: .utf8)
  else { return }
  print(json)
  fflush(stdout)
}
