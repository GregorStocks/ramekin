import Foundation
import SwiftUI

/// A recipe's scale controls as the user left them.
struct RecipeScaleState: Codable, Equatable {
    var scale: Double
    var customInput: String
}

/// Encodes where the user was — navigation stacks and per-recipe scale — for
/// `@SceneStorage`, so relaunching after iOS terminates the app in the
/// background comes back to the same place. Unreadable data restores as
/// nothing rather than failing launch.
enum SceneRestoration {
    static func encodePath(_ path: [NavigationDestination]) -> Data {
        (try? JSONEncoder().encode(path)) ?? Data()
    }

    static func decodePath(_ data: Data) -> [NavigationDestination] {
        (try? JSONDecoder().decode([NavigationDestination].self, from: data)) ?? []
    }

    /// The path cut off at the first recipe that no longer exists, so the
    /// user lands on the screen they opened it from instead of an error.
    static func prunedPath(
        _ path: [NavigationDestination],
        recipeExists: (UUID) -> Bool
    ) -> [NavigationDestination] {
        guard let firstMissing = path.firstIndex(where: { destination in
            if case .recipe(let id) = destination { return !recipeExists(id) }
            return false
        }) else {
            return path
        }
        return Array(path[..<firstMissing])
    }

    static func recipeIds(in path: [NavigationDestination]) -> [UUID] {
        path.compactMap { destination in
            if case .recipe(let id) = destination { return id }
            return nil
        }
    }

    static func encodeScales(_ scales: [UUID: RecipeScaleState]) -> Data {
        (try? JSONEncoder().encode(scales)) ?? Data()
    }

    static func decodeScales(_ data: Data) -> [UUID: RecipeScaleState] {
        (try? JSONDecoder().decode([UUID: RecipeScaleState].self, from: data)) ?? [:]
    }

    /// Only the scales of recipes still open somewhere, so the stored
    /// dictionary doesn't grow with every recipe ever viewed.
    static func prunedScales(
        _ scales: [UUID: RecipeScaleState],
        keeping ids: Set<UUID>
    ) -> [UUID: RecipeScaleState] {
        scales.filter { ids.contains($0.key) }
    }
}

/// Reads and records each open recipe's scale in the scene's restoration
/// state. The default remembers nothing (previews, unit tests).
struct RecipeScaleMemory {
    var stored: (UUID) -> RecipeScaleState? = { _ in nil }
    var remember: (UUID, RecipeScaleState) -> Void = { _, _ in }
}

private struct RecipeScaleMemoryKey: EnvironmentKey {
    static let defaultValue = RecipeScaleMemory()
}

extension EnvironmentValues {
    var recipeScaleMemory: RecipeScaleMemory {
        get { self[RecipeScaleMemoryKey.self] }
        set { self[RecipeScaleMemoryKey.self] = newValue }
    }
}
