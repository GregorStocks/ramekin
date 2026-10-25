import Foundation
import SwiftUI

/// Where the user left an open recipe: its scale controls and the topmost
/// ingredient or step in view (nil means the top of the recipe).
struct RecipePlace: Codable, Equatable {
    var scale: Double = 1
    var customInput: String = ""
    var scrollAnchor: String?
}

/// A recipe form the user closed the app on without saving or discarding.
/// `form.expectedVersionId` is the version an edit started from.
struct RecipeFormDraft: Codable, Equatable {
    var mode: RecipeFormMode
    var form: RecipeFormData
    var recipeText: String = ""
    var rawIngredients: String = ""
    var prepared: PrepareTextRecipeResponse?
}

/// Encodes where the user was — navigation stacks, per-recipe place, and
/// unsaved recipe forms — for
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

    static func encodePlaces(_ places: [UUID: RecipePlace]) -> Data {
        (try? JSONEncoder().encode(places)) ?? Data()
    }

    static func decodePlaces(_ data: Data) -> [UUID: RecipePlace] {
        (try? JSONDecoder().decode([UUID: RecipePlace].self, from: data)) ?? [:]
    }

    /// Only the places of recipes still open somewhere, so the stored
    /// dictionary doesn't grow with every recipe ever viewed.
    static func prunedPlaces(
        _ places: [UUID: RecipePlace],
        keeping ids: Set<UUID>
    ) -> [UUID: RecipePlace] {
        places.filter { ids.contains($0.key) }
    }

    /// The anchor to restore a recipe's scroll position to: the lowest one
    /// whose top is at or above the top of the viewport, so restoring never
    /// skips past content. `positions` maps anchor ids to their top edge in
    /// the scroll view's coordinate space; nil means the top of the recipe.
    static func topAnchor(_ positions: [String: CGFloat]) -> String? {
        // Restoring scrolls an anchor to exactly 0; allow for rounding.
        positions
            .filter { $0.value <= 1 }
            .max { $0.value < $1.value }?
            .key
    }

    static func encodeDrafts(_ drafts: [RecipeFormDraft]) -> Data {
        (try? JSONEncoder().encode(drafts)) ?? Data()
    }

    static func decodeDrafts(_ data: Data) -> [RecipeFormDraft] {
        (try? JSONDecoder().decode([RecipeFormDraft].self, from: data)) ?? []
    }

    /// The drafts with the one for `mode` replaced, or removed when nil.
    static func replacingDraft(
        in drafts: [RecipeFormDraft],
        for mode: RecipeFormMode,
        with draft: RecipeFormDraft?
    ) -> [RecipeFormDraft] {
        drafts.filter { $0.mode != mode } + (draft.map { [$0] } ?? [])
    }

    /// The new-recipe draft and edits of recipes still open somewhere; an edit
    /// is offered for resuming only on its recipe's screen.
    static func prunedDrafts(
        _ drafts: [RecipeFormDraft],
        keeping ids: Set<UUID>
    ) -> [RecipeFormDraft] {
        drafts.filter { draft in
            if case .edit(let recipeId) = draft.mode { return ids.contains(recipeId) }
            return true
        }
    }
}

/// Reads and records each open recipe's place in the scene's restoration
/// state. The default remembers nothing (previews, unit tests).
struct RecipePlaceMemory {
    var stored: (UUID) -> RecipePlace? = { _ in nil }
    var remember: (UUID, RecipePlace) -> Void = { _, _ in }

    func update(_ recipeId: UUID, _ change: (inout RecipePlace) -> Void) {
        var place = stored(recipeId) ?? RecipePlace()
        change(&place)
        remember(recipeId, place)
    }
}

/// Reads and records unsaved recipe forms in the scene's restoration state;
/// remembering nil forgets the form's draft. The default remembers nothing.
struct RecipeFormDraftMemory {
    var stored: (RecipeFormMode) -> RecipeFormDraft? = { _ in nil }
    var remember: (RecipeFormMode, RecipeFormDraft?) -> Void = { _, _ in }
}

private struct RecipePlaceMemoryKey: EnvironmentKey {
    static let defaultValue = RecipePlaceMemory()
}

private struct RecipeFormDraftMemoryKey: EnvironmentKey {
    static let defaultValue = RecipeFormDraftMemory()
}

extension EnvironmentValues {
    var recipePlaceMemory: RecipePlaceMemory {
        get { self[RecipePlaceMemoryKey.self] }
        set { self[RecipePlaceMemoryKey.self] = newValue }
    }

    var recipeFormDraftMemory: RecipeFormDraftMemory {
        get { self[RecipeFormDraftMemoryKey.self] }
        set { self[RecipeFormDraftMemoryKey.self] = newValue }
    }
}
