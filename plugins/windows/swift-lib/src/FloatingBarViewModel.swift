import Combine
import Foundation

final class FloatingBarViewModel: ObservableObject {
  @Published var amplitude: Double = 0
  @Published var degraded = false
}
