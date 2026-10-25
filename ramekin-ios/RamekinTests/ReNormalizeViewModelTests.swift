import XCTest
@testable import Ramekin

final class ReNormalizeViewModelTests: XCTestCase {
    @MainActor
    func testRunFollowsTheJobToItsProposalAndAcceptSavesIt() async {
        let recipeId = UUID()
        let jobId = UUID()
        var statuses = ["normalizing", "completed"]
        var startedOps: [String]?
        var accepted: (UUID, UUID)?
        let base = makeNormalized(title: "Soup!!! The Best")
        let proposed = makeNormalized(title: "Soup")

        let viewModel = ReNormalizeViewModel(
            recipeId: recipeId,
            api: ReNormalizeAPIClient(
                listOps: { id in
                    XCTAssertEqual(id, recipeId)
                    return NormalizeOpsResponse(ops: normalizeTestOps)
                },
                start: { id, ops in
                    XCTAssertEqual(id, recipeId)
                    startedOps = ops
                    return NormalizeJobResponse(jobId: jobId, status: "pending")
                },
                getScrape: { id in
                    XCTAssertEqual(id, jobId)
                    let status = statuses.removeFirst()
                    return makeJob(id: jobId, recipeId: recipeId, status: status)
                },
                getProposal: { _, _ in
                    NormalizeProposalResponse(
                        base: base,
                        baseVersionId: UUID(),
                        ops: ["normalize_title"],
                        proposed: proposed,
                        status: "completed"
                    )
                },
                accept: { id, job in
                    accepted = (id, job)
                    return AcceptNormalizeResponse(versionId: UUID())
                }
            )
        )
        viewModel.pollIntervalNanoseconds = 0

        await viewModel.load()
        XCTAssertEqual(viewModel.phase, .choose)
        XCTAssertEqual(viewModel.selected, ["normalize_title", "generate_description", "auto_tag"])

        viewModel.selected = ["normalize_title"]
        await viewModel.run()

        XCTAssertEqual(startedOps, ["normalize_title"])
        XCTAssertEqual(viewModel.phase, .review)
        XCTAssertEqual(
            viewModel.diffs,
            [RecipeContentFieldDiff(label: "Title", before: "Soup!!! The Best", after: "Soup")]
        )
        XCTAssertFalse(viewModel.photosChange)
        XCTAssertTrue(viewModel.hasChanges)

        let saved = await viewModel.accept()

        XCTAssertTrue(saved)
        XCTAssertEqual(accepted?.0, recipeId)
        XCTAssertEqual(accepted?.1, jobId)
    }

    @MainActor
    func testFailedJobShowsItsError() async {
        let jobId = UUID()
        let viewModel = ReNormalizeViewModel(
            recipeId: UUID(),
            api: ReNormalizeAPIClient(
                listOps: { _ in NormalizeOpsResponse(ops: normalizeTestOps) },
                start: { _, _ in NormalizeJobResponse(jobId: jobId, status: "pending") },
                getScrape: { _ in
                    makeJob(id: jobId, recipeId: nil, status: "failed", error: "Page not found")
                },
                getProposal: { _, _ in throw URLError(.badServerResponse) },
                accept: { _, _ in throw URLError(.badServerResponse) }
            )
        )
        viewModel.pollIntervalNanoseconds = 0

        await viewModel.load()
        await viewModel.run()

        XCTAssertEqual(viewModel.phase, .failed("Page not found"))
    }

    @MainActor
    func testFailedAcceptKeepsTheReviewOpen() async {
        let jobId = UUID()
        let viewModel = ReNormalizeViewModel(
            recipeId: UUID(),
            api: ReNormalizeAPIClient(
                listOps: { _ in NormalizeOpsResponse(ops: normalizeTestOps) },
                start: { _, _ in NormalizeJobResponse(jobId: jobId, status: "pending") },
                getScrape: { _ in makeJob(id: jobId, recipeId: nil, status: "completed") },
                getProposal: { _, _ in
                    NormalizeProposalResponse(
                        base: makeNormalized(title: "A"),
                        baseVersionId: UUID(),
                        ops: ["normalize_title"],
                        proposed: makeNormalized(title: "B"),
                        status: "completed"
                    )
                },
                accept: { _, _ in throw URLError(.timedOut) }
            )
        )
        viewModel.pollIntervalNanoseconds = 0

        await viewModel.load()
        await viewModel.run()
        let saved = await viewModel.accept()

        XCTAssertFalse(saved)
        XCTAssertEqual(viewModel.phase, .review)
        XCTAssertNotNil(viewModel.error)
    }
}

private func makeNormalized(title: String) -> NormalizedRecipe {
    NormalizedRecipe(
        content: RecipeContent(ingredients: [], instructions: "Simmer.", title: title),
        photoIds: []
    )
}

private func makeJob(id: UUID, recipeId: UUID?, status: String, error: String? = nil) -> ScrapeJobResponse {
    ScrapeJobResponse(
        canRetry: status == "failed",
        createdAt: Date(timeIntervalSince1970: 1),
        error: error,
        id: id,
        recipeId: recipeId,
        retryCount: 0,
        status: status,
        steps: []
    )
}
