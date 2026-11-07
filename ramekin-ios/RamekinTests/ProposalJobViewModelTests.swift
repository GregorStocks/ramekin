import XCTest
@testable import Ramekin

final class ProposalJobViewModelTests: XCTestCase {
    @MainActor
    func testRunFollowsTheJobToItsProposalAndAcceptSavesIt() async {
        let recipeId = UUID()
        let jobId = UUID()
        var statuses = ["normalizing", "completed"]
        var startedOps: [String]?
        var accepted: (UUID, UUID)?
        let base = makeNormalized(title: "Soup!!! The Best")
        let proposed = makeNormalized(title: "Soup")

        let viewModel = ProposalJobViewModel(
            recipeId: recipeId,
            mode: .normalize,
            api: ProposalJobAPIClient(
                listOps: { id in
                    XCTAssertEqual(id, recipeId)
                    return NormalizeOpsResponse(ops: normalizeTestOps)
                },
                start: { id, ops in
                    XCTAssertEqual(id, recipeId)
                    startedOps = ops
                    return NormalizeJobResponse(jobId: jobId, status: .pending)
                },
                startEdit: { _, _, _ in throw URLError(.badServerResponse) },
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
                        status: .completed
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
        XCTAssertEqual(viewModel.phase, .choose(error: nil))
        XCTAssertEqual(viewModel.selected, ["normalize_title", "generate_description", "auto_tag"])

        viewModel.selected = ["normalize_title"]
        await viewModel.run()

        XCTAssertEqual(startedOps, ["normalize_title"])
        XCTAssertTrue(viewModel.isReviewing)
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
        let viewModel = ProposalJobViewModel(
            recipeId: UUID(),
            mode: .normalize,
            api: ProposalJobAPIClient(
                listOps: { _ in NormalizeOpsResponse(ops: normalizeTestOps) },
                start: { _, _ in NormalizeJobResponse(jobId: jobId, status: .pending) },
                startEdit: { _, _, _ in throw URLError(.badServerResponse) },
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
        let viewModel = ProposalJobViewModel(
            recipeId: UUID(),
            mode: .normalize,
            api: ProposalJobAPIClient(
                listOps: { _ in NormalizeOpsResponse(ops: normalizeTestOps) },
                start: { _, _ in NormalizeJobResponse(jobId: jobId, status: .pending) },
                startEdit: { _, _, _ in throw URLError(.badServerResponse) },
                getScrape: { _ in makeJob(id: jobId, recipeId: nil, status: "completed") },
                getProposal: { _, _ in
                    NormalizeProposalResponse(
                        base: makeNormalized(title: "A"),
                        baseVersionId: UUID(),
                        ops: ["normalize_title"],
                        proposed: makeNormalized(title: "B"),
                        status: .completed
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
        guard case .review(_, _, let isAccepting, let error) = viewModel.phase else {
            return XCTFail("Expected the review to stay open, got \(viewModel.phase)")
        }
        XCTAssertFalse(isAccepting)
        XCTAssertNotNil(error)
        XCTAssertFalse(viewModel.isBusy)
    }

    @MainActor
    func testFailedOpsLoadOffersRetry() async {
        var attempts = 0
        let viewModel = ProposalJobViewModel(
            recipeId: UUID(),
            mode: .normalize,
            api: ProposalJobAPIClient(
                listOps: { _ in
                    attempts += 1
                    if attempts == 1 { throw URLError(.timedOut) }
                    return NormalizeOpsResponse(ops: normalizeTestOps)
                },
                start: { _, _ in throw URLError(.badServerResponse) },
                startEdit: { _, _, _ in throw URLError(.badServerResponse) },
                getScrape: { _ in throw URLError(.badServerResponse) },
                getProposal: { _, _ in throw URLError(.badServerResponse) },
                accept: { _, _ in throw URLError(.badServerResponse) }
            )
        )

        await viewModel.load()
        guard case .loadFailed = viewModel.phase else {
            return XCTFail("Expected a failed load, got \(viewModel.phase)")
        }
        XCTAssertFalse(viewModel.canRun)

        await viewModel.load()
        XCTAssertEqual(viewModel.phase, .choose(error: nil))
        XCTAssertFalse(viewModel.ops.isEmpty)
    }

    @MainActor
    func testFailedStartReturnsToChoosingWithTheError() async {
        let viewModel = ProposalJobViewModel(
            recipeId: UUID(),
            mode: .normalize,
            api: ProposalJobAPIClient(
                listOps: { _ in NormalizeOpsResponse(ops: normalizeTestOps) },
                start: { _, _ in throw URLError(.timedOut) },
                startEdit: { _, _, _ in throw URLError(.badServerResponse) },
                getScrape: { _ in throw URLError(.badServerResponse) },
                getProposal: { _, _ in throw URLError(.badServerResponse) },
                accept: { _, _ in throw URLError(.badServerResponse) }
            )
        )

        await viewModel.load()
        await viewModel.run()

        guard case .choose(let error) = viewModel.phase else {
            return XCTFail("Expected to be choosing again, got \(viewModel.phase)")
        }
        XCTAssertNotNil(error)
        XCTAssertTrue(viewModel.canRun)
    }

    @MainActor
    func testTryAgainAfterAFailedJobGoesBackToChoosing() async {
        let jobId = UUID()
        let viewModel = ProposalJobViewModel(
            recipeId: UUID(),
            mode: .normalize,
            api: ProposalJobAPIClient(
                listOps: { _ in NormalizeOpsResponse(ops: normalizeTestOps) },
                start: { _, _ in NormalizeJobResponse(jobId: jobId, status: .pending) },
                startEdit: { _, _, _ in throw URLError(.badServerResponse) },
                getScrape: { _ in
                    makeJob(id: jobId, recipeId: nil, status: "failed", error: "Page not found")
                },
                getProposal: { _, _ in throw URLError(.badServerResponse) },
                accept: { _, _ in throw URLError(.badServerResponse) }
            )
        )
        viewModel.pollIntervalNanoseconds = 0

        await viewModel.load()
        viewModel.selected = ["normalize_title"]
        await viewModel.run()
        viewModel.tryAgain()

        XCTAssertEqual(viewModel.phase, .choose(error: nil))
        XCTAssertEqual(viewModel.selected, ["normalize_title"])
    }

    @MainActor
    func testAcceptIsBusyUntilTheSaveAnswers() async {
        let jobId = UUID()
        var acceptContinuation: CheckedContinuation<AcceptNormalizeResponse, Never>?
        let viewModel = ProposalJobViewModel(
            recipeId: UUID(),
            mode: .normalize,
            api: ProposalJobAPIClient(
                listOps: { _ in NormalizeOpsResponse(ops: normalizeTestOps) },
                start: { _, _ in NormalizeJobResponse(jobId: jobId, status: .pending) },
                startEdit: { _, _, _ in throw URLError(.badServerResponse) },
                getScrape: { _ in makeJob(id: jobId, recipeId: nil, status: "completed") },
                getProposal: { _, _ in
                    NormalizeProposalResponse(
                        base: makeNormalized(title: "A"),
                        baseVersionId: UUID(),
                        ops: ["normalize_title"],
                        proposed: makeNormalized(title: "B"),
                        status: .completed
                    )
                },
                accept: { _, _ in
                    await withCheckedContinuation { acceptContinuation = $0 }
                }
            )
        )
        viewModel.pollIntervalNanoseconds = 0

        await viewModel.load()
        await viewModel.run()
        let acceptTask = Task { await viewModel.accept() }
        while acceptContinuation == nil {
            await Task.yield()
        }

        XCTAssertTrue(viewModel.isBusy)
        let secondAccept = await viewModel.accept()
        XCTAssertFalse(secondAccept, "A second Accept while saving does nothing")

        acceptContinuation?.resume(returning: AcceptNormalizeResponse(versionId: UUID()))
        let saved = await acceptTask.value
        XCTAssertTrue(saved)
    }
}

extension ProposalJobViewModelTests {
    @MainActor
    func testEditModeSendsTheInstructionAndPhotosAndShowsTheSummary() async {
        let recipeId = UUID()
        let jobId = UUID()
        let photoId = UUID()
        var sent: (String, [UUID])?
        let viewModel = ProposalJobViewModel(
            recipeId: recipeId,
            mode: .edit(photoIds: [photoId]),
            api: ProposalJobAPIClient(
                listOps: { _ in throw URLError(.badServerResponse) },
                start: { _, _ in throw URLError(.badServerResponse) },
                startEdit: { id, instruction, photoIds in
                    XCTAssertEqual(id, recipeId)
                    sent = (instruction, photoIds)
                    return NormalizeJobResponse(jobId: jobId, status: .pending)
                },
                getScrape: { _ in makeJob(id: jobId, recipeId: recipeId, status: "completed") },
                getProposal: { _, _ in
                    NormalizeProposalResponse(
                        base: makeNormalized(title: "Soup"),
                        baseVersionId: UUID(),
                        ops: [],
                        proposed: makeNormalized(title: "Vegan Soup"),
                        status: .completed,
                        summary: "Made it vegan."
                    )
                },
                accept: { _, _ in AcceptNormalizeResponse(versionId: UUID()) }
            )
        )
        viewModel.pollIntervalNanoseconds = 0

        await viewModel.load()
        XCTAssertEqual(viewModel.phase, .choose(error: nil))
        XCTAssertFalse(viewModel.canRun)
        viewModel.instruction = "  make it vegan  "
        XCTAssertTrue(viewModel.canRun)
        await viewModel.run()

        XCTAssertEqual(sent?.0, "make it vegan")
        XCTAssertEqual(sent?.1, [photoId])
        XCTAssertTrue(viewModel.isReviewing)
        XCTAssertEqual(viewModel.proposal?.summary, "Made it vegan.")
        XCTAssertTrue(viewModel.hasChanges)
    }
}

private extension ProposalJobViewModel {
    var isReviewing: Bool {
        guard case .review = phase else { return false }
        return true
    }
}

private func makeNormalized(title: String) -> NormalizedRecipe {
    NormalizedRecipe(
        content: RecipeContent(ingredients: [], instructions: "Simmer.", title: title),
        photoIds: []
    )
}

/// A job as the server reports it, with `isTerminal` and `recipeReady`
/// following the server's rules for `status`.
private func makeJob(
    id: UUID,
    recipeId: UUID?,
    status rawStatus: String,
    error: String? = nil
) -> ScrapeJobResponse {
    let status = ScrapeJobStatus(rawValue: rawStatus)!
    let stopped = status == .completed || status == .failed
    return ScrapeJobResponse(
        canRetry: status == .failed,
        createdAt: Date(timeIntervalSince1970: 1),
        error: error,
        id: id,
        isTerminal: stopped,
        recipeId: recipeId,
        recipeReady: recipeId != nil && (stopped || status == .normalizing),
        retryCount: 0,
        status: status,
        steps: []
    )
}
