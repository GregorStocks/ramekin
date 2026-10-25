import XCTest
@testable import Ramekin

// Mirrors ramekin-ui/src/utils/normalizeSelection.test.ts.

func makeNormalizeOp(
    _ id: String,
    label: String? = nil,
    group: NormalizeOpGroupResponse = .normalize,
    implies: [String] = [],
    defaultOnImport: Bool = true,
    available: Bool = true,
    unavailableReason: String? = nil
) -> NormalizeOpInfo {
    NormalizeOpInfo(
        available: available,
        defaultOnImport: defaultOnImport,
        group: group,
        id: id,
        implies: implies,
        label: label ?? id,
        runsAfter: [],
        unavailableReason: unavailableReason,
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

final class NormalizeSelectionTests: XCTestCase {
    func testStartsWithTheOpsImportsRun() {
        XCTAssertEqual(
            NormalizeSelection.defaultSelection(normalizeTestOps),
            ["normalize_title", "generate_description", "auto_tag"]
        )
    }

    func testLeavesUnavailableDefaultOpsUnchecked() {
        let ops = normalizeTestOps.map { opInfo in
            opInfo.id == "auto_tag" ? makeNormalizeOp("auto_tag", available: false) : opInfo
        }
        XCTAssertFalse(NormalizeSelection.defaultSelection(ops).contains("auto_tag"))
    }

    func testCheckingRefetchChecksAndLocksReextract() {
        let ops = normalizeTestOps
        let selected = NormalizeSelection.toggle(ops, selected: [], id: "refetch")
        XCTAssertEqual(selected, ["refetch"])
        XCTAssertTrue(NormalizeSelection.locked(ops, selected: selected).contains("reextract"))
        XCTAssertEqual(NormalizeSelection.effective(ops, selected: selected), ["refetch", "reextract"])
        // Re-extract can't be unchecked while re-fetch is checked.
        XCTAssertEqual(NormalizeSelection.toggle(ops, selected: selected, id: "reextract"), selected)
    }

    func testUncheckingRefetchUnlocksReextract() {
        let selected = NormalizeSelection.toggle(normalizeTestOps, selected: ["refetch"], id: "refetch")
        XCTAssertTrue(selected.isEmpty)
        XCTAssertEqual(NormalizeSelection.effective(normalizeTestOps, selected: selected), [])
    }

    func testDoesNotCheckAnUnavailableOp() {
        let ops = normalizeTestOps.map { opInfo in
            opInfo.id == "refetch"
                ? makeNormalizeOp(
                    "refetch",
                    group: .source,
                    implies: ["reextract"],
                    defaultOnImport: false,
                    available: false,
                    unavailableReason: "No source URL"
                )
                : opInfo
        }
        XCTAssertEqual(NormalizeSelection.toggle(ops, selected: [], id: "refetch"), [])
    }

    func testListsEffectiveOpsInRegistryOrder() {
        XCTAssertEqual(
            NormalizeSelection.effective(normalizeTestOps, selected: ["auto_tag", "normalize_title"]),
            ["normalize_title", "auto_tag"]
        )
    }

    func testLabelsOpStepsByTheirOpAndOtherStepsByWhatTheyDo() {
        XCTAssertEqual(NormalizeSelection.stepLabel("normalize_title", ops: normalizeTestOps), "Normalize title")
        XCTAssertEqual(NormalizeSelection.stepLabel("fetch_html", ops: normalizeTestOps), "Fetching the page")
        XCTAssertEqual(NormalizeSelection.stepLabel("some_new_step", ops: normalizeTestOps), "some new step")
    }
}
