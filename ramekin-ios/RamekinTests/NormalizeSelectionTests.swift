import XCTest
@testable import Ramekin

func makeNormalizeOp(
    _ id: String,
    label: String? = nil,
    group: NormalizeOpGroupResponse = .normalize,
    implies: [String] = [],
    defaultOnImport: Bool = true,
    available: Bool = true
) -> NormalizeOpInfo {
    NormalizeOpInfo(
        available: available,
        defaultOnImport: defaultOnImport,
        group: group,
        id: id,
        implies: implies,
        label: label ?? id,
        runsAfter: [],
        unavailableReason: available ? nil : "Unavailable",
        usesAi: group == .normalize,
        writes: []
    )
}

let normalizeTestOps: [NormalizeOpInfo] = [
    makeNormalizeOp(
        "refetch",
        label: "Re-fetch from source",
        group: .source,
        implies: ["reextract"],
        defaultOnImport: false
    ),
    makeNormalizeOp(
        "reextract",
        label: "Re-extract from saved source",
        group: .source,
        defaultOnImport: false
    ),
    makeNormalizeOp("normalize_title", label: "Normalize title"),
    makeNormalizeOp("generate_description", label: "Generate description"),
    makeNormalizeOp("auto_tag", label: "Auto-tag")
]

private struct NormalizeVectorOp: Decodable {
    let id: String
    let label: String
    let group: NormalizeOpGroupResponse
    let implies: [String]
    let defaultOnImport: Bool

    private enum CodingKeys: String, CodingKey {
        case id, label, group, implies
        case defaultOnImport = "default_on_import"
    }
}

private struct NormalizeDefaultSelectionVector: Decodable {
    let name: String
    let unavailable: [String]
    let expected: [String]
}

private struct NormalizeToggleVector: Decodable {
    let name: String
    let unavailable: [String]
    let selected: [String]
    let toggle: String
    let expectedSelected: [String]
    let expectedLocked: [String]
    let expectedEffective: [String]

    private enum CodingKeys: String, CodingKey {
        case name, unavailable, selected, toggle
        case expectedSelected = "expected_selected"
        case expectedLocked = "expected_locked"
        case expectedEffective = "expected_effective"
    }
}

private struct NormalizeStepLabelVector: Decodable {
    let step: String
    let expected: String
}

private struct NormalizeProposalChangeVector: Decodable {
    let name: String
    let base: NormalizedRecipe
    let proposed: NormalizedRecipe
    let expectedChanged: Bool
    let expectedLabels: [String]

    private enum CodingKeys: String, CodingKey {
        case name, base, proposed
        case expectedChanged = "expected_changed"
        case expectedLabels = "expected_labels"
    }
}

private struct NormalizeVectors: Decodable {
    let ops: [NormalizeVectorOp]
    let defaultSelection: [NormalizeDefaultSelectionVector]
    let toggle: [NormalizeToggleVector]
    let stepLabels: [NormalizeStepLabelVector]
    let proposalChanges: [NormalizeProposalChangeVector]

    private enum CodingKeys: String, CodingKey {
        case ops
        case defaultSelection = "default_selection"
        case toggle
        case stepLabels = "step_labels"
        case proposalChanges = "proposal_changes"
    }
}

/// Runs shared-test-vectors/normalize-selection.json, which the web tests
/// (ramekin-ui/src/utils/normalizeSelection.test.ts) run too.
final class NormalizeSelectionTests: XCTestCase {
    private func loadVectors() throws -> NormalizeVectors {
        let url = try XCTUnwrap(
            Bundle(for: Self.self).url(forResource: "normalize-selection", withExtension: "json")
        )
        return try JSONDecoder().decode(NormalizeVectors.self, from: Data(contentsOf: url))
    }

    private func ops(_ vectors: NormalizeVectors, unavailable: [String] = []) -> [NormalizeOpInfo] {
        vectors.ops.map { vector in
            makeNormalizeOp(
                vector.id,
                label: vector.label,
                group: vector.group,
                implies: vector.implies,
                defaultOnImport: vector.defaultOnImport,
                available: !unavailable.contains(vector.id)
            )
        }
    }

    /// A set's members in registry order, for comparing with the vectors.
    private func inRegistryOrder(_ set: Set<String>, _ vectors: NormalizeVectors) -> [String] {
        vectors.ops.map(\.id).filter { set.contains($0) }
    }

    func testDefaultSelectionMatchesSharedVectors() throws {
        let vectors = try loadVectors()
        for vector in vectors.defaultSelection {
            let selected = NormalizeSelection.defaultSelection(ops(vectors, unavailable: vector.unavailable))
            XCTAssertEqual(inRegistryOrder(selected, vectors), vector.expected, vector.name)
        }
    }

    func testToggleMatchesSharedVectors() throws {
        let vectors = try loadVectors()
        for vector in vectors.toggle {
            let available = ops(vectors, unavailable: vector.unavailable)
            let selected = NormalizeSelection.toggle(available, selected: Set(vector.selected), id: vector.toggle)
            XCTAssertEqual(inRegistryOrder(selected, vectors), vector.expectedSelected, vector.name)
            XCTAssertEqual(
                inRegistryOrder(NormalizeSelection.locked(available, selected: selected), vectors),
                vector.expectedLocked,
                vector.name
            )
            XCTAssertEqual(
                NormalizeSelection.effective(available, selected: selected),
                vector.expectedEffective,
                vector.name
            )
        }
    }

    func testStepLabelsMatchSharedVectors() throws {
        let vectors = try loadVectors()
        for vector in vectors.stepLabels {
            XCTAssertEqual(NormalizeSelection.stepLabel(vector.step, ops: ops(vectors)), vector.expected, vector.step)
        }
    }

    func testProposalChangesMatchSharedVectors() throws {
        let vectors = try loadVectors()
        for vector in vectors.proposalChanges {
            let diffs = RecipeContentFieldDiff.diffs(old: vector.base.content, new: vector.proposed.content)
            XCTAssertEqual(
                RecipeContentFieldDiff.changed(base: vector.base, proposed: vector.proposed),
                vector.expectedChanged,
                vector.name
            )
            XCTAssertEqual(diffs.map(\.label), vector.expectedLabels, vector.name)
        }
    }
}
