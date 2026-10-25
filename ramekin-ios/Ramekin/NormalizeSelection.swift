import Foundation

/// Which normalize ops the Re-normalize sheet has checked.
///
/// Mirrors ramekin-ui/src/utils/normalizeSelection.ts; pinned by
/// shared-test-vectors/normalize-selection.json.
enum NormalizeSelection {
    /// The ops imports run, where the recipe can run them.
    static func defaultSelection(_ ops: [NormalizeOpInfo]) -> Set<String> {
        Set(ops.filter { $0.defaultOnImport && $0.available }.map(\.id))
    }

    /// Ops some checked op implies, which stay checked and can't be unchecked.
    static func locked(_ ops: [NormalizeOpInfo], selected: Set<String>) -> Set<String> {
        Set(ops.filter { selected.contains($0.id) }.flatMap(\.implies))
    }

    /// The checked ops plus the ones they imply, in the registry's run order.
    static func effective(_ ops: [NormalizeOpInfo], selected: Set<String>) -> [String] {
        let locked = locked(ops, selected: selected)
        return ops
            .filter { selected.contains($0.id) || locked.contains($0.id) }
            .map(\.id)
    }

    /// Checks or unchecks an op. Unavailable ops and ops locked by an implying
    /// op don't change.
    static func toggle(_ ops: [NormalizeOpInfo], selected: Set<String>, id: String) -> Set<String> {
        guard let opInfo = ops.first(where: { $0.id == id }),
              opInfo.available,
              !locked(ops, selected: selected).contains(id) else {
            return selected
        }
        var next = selected
        if next.contains(id) {
            next.remove(id)
        } else {
            next.insert(id)
        }
        return next
    }

    private static let stepLabels: [String: String] = [
        "fetch_html": "Fetching the page",
        "extract_recipe": "Extracting the recipe",
        "parse_ingredients": "Parsing ingredients",
        "fetch_images": "Fetching photos",
        "load_recipe": "Loading the recipe",
        "resolve_ingredient_names": "Matching ingredients",
        "propose_normalize": "Preparing the preview"
    ]

    /// A job step's name as the progress list shows it.
    static func stepLabel(_ stepName: String, ops: [NormalizeOpInfo]) -> String {
        ops.first(where: { $0.id == stepName })?.label
            ?? stepLabels[stepName]
            ?? stepName.replacingOccurrences(of: "_", with: " ")
    }
}
