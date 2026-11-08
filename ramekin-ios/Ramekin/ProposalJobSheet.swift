import SwiftUI

struct ProposalJobAPIClient {
    var listOps: (UUID) async throws -> NormalizeOpsResponse
    var start: (UUID, [String]) async throws -> NormalizeJobResponse
    var startEdit: (UUID, String, [UUID]) async throws -> NormalizeJobResponse
    var getScrape: (UUID) async throws -> ScrapeJobResponse
    var getProposal: (UUID, UUID) async throws -> NormalizeProposalResponse
    var accept: (UUID, UUID) async throws -> AcceptNormalizeResponse

    static let live = ProposalJobAPIClient(
        listOps: { try await NormalizeAPI.listNormalizeOps(recipeId: $0) },
        start: { try await NormalizeAPI.normalizeRecipe(id: $0, normalizeRequest: NormalizeRequest(ops: $1)) },
        startEdit: { recipeId, instruction, photoIds in
            try await EditRecipeAPI.editRecipeWithAi(
                id: recipeId,
                editRecipeRequest: EditRecipeRequest(instruction: instruction, photoIds: photoIds)
            )
        },
        getScrape: { try await ScrapeAPI.getScrape(id: $0) },
        getProposal: { try await NormalizeAPI.getNormalizeProposal(id: $0, jobId: $1) },
        accept: { try await NormalizeAPI.acceptNormalizeProposal(id: $0, jobId: $1) }
    )
}

/// What a proposal job does: normalize ops the user picks, or an Edit with
/// AI run on the user's instruction, with the recipe's photos as context.
enum ProposalJobMode: Equatable {
    case normalize
    case edit(photoIds: [UUID])
}

/// Re-normalize or Edit with AI on one recipe: pick ops (or describe the
/// change), run them as a job, then review the proposed version against the
/// one it started from.
@MainActor
final class ProposalJobViewModel: ObservableObject {
    /// Each phase carries its own data and errors, so nothing from one phase
    /// (a proposal, a failed save) can show in another.
    enum Phase: Equatable {
        case loading
        /// The normalize ops didn't load; retrying loads them again.
        case loadFailed(String)
        /// Picking ops or writing the instruction. `error` says why the last
        /// Run didn't start.
        case choose(error: String?)
        case starting
        case running(jobId: UUID, steps: [StepState])
        /// `error` says why the last Accept didn't save.
        case review(jobId: UUID, proposal: NormalizeProposalResponse, isAccepting: Bool, error: String?)
        /// The job failed or was lost; trying again goes back to choosing.
        case failed(String)
    }

    let recipeId: UUID
    let mode: ProposalJobMode
    @Published var ops: [NormalizeOpInfo] = []
    @Published var instruction = ""
    @Published var selected: Set<String> = []
    @Published private(set) var phase: Phase = .loading
    var pollIntervalNanoseconds: UInt64 = 1_000_000_000

    private let api: ProposalJobAPIClient

    init(recipeId: UUID, mode: ProposalJobMode, api: ProposalJobAPIClient = .live) {
        self.recipeId = recipeId
        self.mode = mode
        self.api = api
    }

    var canRun: Bool {
        guard case .choose = phase else { return false }
        switch mode {
        case .normalize:
            return !effectiveOps.isEmpty
        case .edit:
            return !instruction.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        }
    }

    /// Starting a job or saving its proposal: the choices are locked and the
    /// sheet stays open until it answers.
    var isBusy: Bool {
        switch phase {
        case .starting: return true
        case .review(_, _, let isAccepting, _): return isAccepting
        default: return false
        }
    }

    var steps: [StepState] {
        guard case .running(_, let steps) = phase else { return [] }
        return steps
    }

    var proposal: NormalizeProposalResponse? {
        guard case .review(_, let proposal, _, _) = phase else { return nil }
        return proposal
    }

    var effectiveOps: [String] {
        NormalizeSelection.effective(ops, selected: selected)
    }

    var lockedOps: Set<String> {
        NormalizeSelection.locked(ops, selected: selected)
    }

    /// The checked ops that imply `id`, by label.
    func implyingLabels(_ id: String) -> String {
        ops.filter { selected.contains($0.id) && $0.implies.contains(id) }
            .map(\.label)
            .joined(separator: ", ")
    }

    var diffs: [RecipeContentFieldDiff] {
        guard let proposal, let proposed = proposal.proposed else { return [] }
        return RecipeContentFieldDiff.diffs(old: proposal.base.content, new: proposed.content)
    }

    var photosChange: Bool {
        guard let proposal, let proposed = proposal.proposed else { return false }
        return proposed.photoIds != proposal.base.photoIds
    }

    /// Whether accepting would save anything different.
    var hasChanges: Bool {
        guard let proposal, let proposed = proposal.proposed else { return false }
        return RecipeContentFieldDiff.changed(base: proposal.base, proposed: proposed)
    }

    func load() async {
        if case .edit = mode {
            phase = .choose(error: nil)
            return
        }
        phase = .loading
        do {
            let response = try await api.listOps(recipeId)
            ops = response.ops
            selected = NormalizeSelection.defaultSelection(response.ops)
            phase = .choose(error: nil)
        } catch is CancellationError {
            return
        } catch {
            phase = .loadFailed(APIErrorFormatter.userMessage(from: error, fallback: "Failed to load normalize ops"))
        }
    }

    /// Back to choosing after a failed job, keeping the choices made.
    func tryAgain() {
        guard case .failed = phase else { return }
        phase = .choose(error: nil)
    }

    func toggle(_ id: String) {
        selected = NormalizeSelection.toggle(ops, selected: selected, id: id)
    }

    private func startJob() async throws -> UUID {
        switch mode {
        case .normalize:
            return try await api.start(recipeId, effectiveOps).jobId
        case .edit(let photoIds):
            let instruction = instruction.trimmingCharacters(in: .whitespacesAndNewlines)
            return try await api.startEdit(recipeId, instruction, photoIds).jobId
        }
    }

    /// Starts the job and follows it to its proposal.
    func run() async {
        guard canRun else { return }
        phase = .starting
        let jobId: UUID
        do {
            jobId = try await startJob()
        } catch is CancellationError {
            phase = .choose(error: nil)
            return
        } catch {
            phase = .choose(error: APIErrorFormatter.userMessage(from: error, fallback: "Failed to start"))
            return
        }
        phase = .running(jobId: jobId, steps: [])

        do {
            // AI can be slow, so there's no client deadline (the server
            // fails a step that runs past its own); closing the sheet
            // cancels the task.
            let poller = ScrapeJobPoller(getScrape: api.getScrape, intervalNanoseconds: pollIntervalNanoseconds)
            switch try await poller.poll(jobId, onUpdate: { phase = .running(jobId: jobId, steps: $0.steps) }) {
            case .completed, .recipeReady:
                let proposal = try await api.getProposal(recipeId, jobId)
                phase = .review(jobId: jobId, proposal: proposal, isAccepting: false, error: nil)
            case .failed(let job):
                phase = .failed(job.error ?? "The job failed")
            }
        } catch is CancellationError {
            return
        } catch {
            phase = .failed(APIErrorFormatter.userMessage(from: error, fallback: "Lost track of the job"))
        }
    }

    /// Saves the proposal as the recipe's new version; returns whether it did.
    func accept() async -> Bool {
        guard case .review(let jobId, let proposal, false, _) = phase else { return false }
        phase = .review(jobId: jobId, proposal: proposal, isAccepting: true, error: nil)
        let message: String?
        do {
            _ = try await api.accept(recipeId, jobId)
            // Stays accepting: the sheet closes once the recipe reloads.
            return true
        } catch is CancellationError {
            message = nil
        } catch {
            message = APIErrorFormatter.userMessage(from: error, fallback: "Failed to save the new version")
        }
        // Only this review's own save may reopen it for another try.
        guard case .review(jobId, _, true, _) = phase else { return false }
        phase = .review(jobId: jobId, proposal: proposal, isAccepting: false, error: message)
        return false
    }
}

struct ProposalJobSheet: View {
    @StateObject private var viewModel: ProposalJobViewModel
    @Binding var isPresented: Bool
    /// The proposal was saved as the recipe's new version.
    let onAccepted: () async -> Void
    @State private var runTask: Task<Void, Never>?
    @State private var acceptTask: Task<Void, Never>?

    init(
        recipeId: UUID,
        mode: ProposalJobMode,
        isPresented: Binding<Bool>,
        api: ProposalJobAPIClient = .live,
        onAccepted: @escaping () async -> Void
    ) {
        _viewModel = StateObject(wrappedValue: ProposalJobViewModel(recipeId: recipeId, mode: mode, api: api))
        _isPresented = isPresented
        self.onAccepted = onAccepted
    }

    var body: some View {
        NavigationStack {
            Form {
                content
            }
            .navigationTitle(viewModel.mode == .normalize ? "Re-normalize" : "Edit with AI")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { toolbar }
            .interactiveDismissDisabled(viewModel.isBusy)
        }
        .task { await viewModel.load() }
        // Closing the sheet abandons the job; it proposes, so nothing is saved.
        .onDisappear {
            runTask?.cancel()
            acceptTask?.cancel()
        }
    }

    @ViewBuilder
    private var content: some View {
        switch viewModel.phase {
        case .loading:
            Section {
                ProgressView().frame(maxWidth: .infinity)
            }
        case .loadFailed(let message):
            Section {
                Text(message).foregroundColor(.red)
                Button("Retry") {
                    Task { await viewModel.load() }
                }
            }
        case .choose(let error):
            errorSection(error)
            chooseContent
        case .starting:
            chooseContent
        case .running:
            Section("Progress") {
                if viewModel.steps.isEmpty {
                    Text("Starting…")
                }
                ForEach(viewModel.steps, id: \.name) { step in
                    stepRow(step)
                }
            }
        case .review(_, let proposal, _, let error):
            errorSection(error)
            reviewContent(proposal)
        case .failed(let message):
            Section {
                Text(message).foregroundColor(.red)
                Button("Try Again") { viewModel.tryAgain() }
            }
        }
    }

    @ViewBuilder
    private func errorSection(_ error: String?) -> some View {
        if let error {
            Section {
                Text(error).foregroundColor(.red)
            }
        }
    }

    @ViewBuilder
    private var chooseContent: some View {
        if viewModel.mode == .normalize {
            Section {
                Text("Source ops replace the recipe's content, hand edits included. You review the result before it's saved.")
                    .font(.footnote)
                    .foregroundColor(.secondary)
            }
            opSection(.source, title: "Source")
            opSection(.normalize, title: "Normalize")
        } else {
            Section("What would you like to change?") {
                TextField(
                    "e.g., make this vegan, convert to grams, cut it to 500 kcal a serving…",
                    text: $viewModel.instruction,
                    axis: .vertical
                )
                .lineLimit(2...5)
                .disabled(viewModel.isBusy)
            }
            Section {
                Text("The AI looks up ingredient weights and calories as it works. You review the result before it's saved.")
                    .font(.footnote)
                    .foregroundColor(.secondary)
            }
        }
    }

    @ViewBuilder
    private func reviewContent(_ proposal: NormalizeProposalResponse) -> some View {
        if let summary = proposal.summary, !summary.isEmpty {
            Section("Summary") {
                Text(summary)
            }
        }
        if viewModel.photosChange {
            Section {
                Text("The recipe's photos change.")
                    .foregroundColor(.secondary)
            }
        }
        Section("Changes") {
            RecipeContentDiffView(
                diffs: viewModel.diffs,
                emptyMessage: viewModel.photosChange
                    ? "No other fields change."
                    : "Nothing changes, so there is nothing to save."
            )
        }
    }

    @ViewBuilder
    private func opSection(_ group: NormalizeOpGroupResponse, title: String) -> some View {
        let ops = viewModel.ops.filter { $0.group == group }
        if !ops.isEmpty {
            Section(title) {
                ForEach(ops, id: \.id) { opInfo in
                    opRow(opInfo)
                }
            }
        }
    }

    private func opRow(_ opInfo: NormalizeOpInfo) -> some View {
        let locked = viewModel.lockedOps.contains(opInfo.id)
        return Toggle(isOn: Binding(
            get: { viewModel.selected.contains(opInfo.id) || locked },
            set: { _ in viewModel.toggle(opInfo.id) }
        )) {
            VStack(alignment: .leading, spacing: 2) {
                Text(opInfo.label)
                if !opInfo.available, let reason = opInfo.unavailableReason {
                    Text(reason)
                        .font(.caption)
                        .foregroundColor(.secondary)
                } else if locked {
                    Text("Included with \(viewModel.implyingLabels(opInfo.id))")
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
            }
        }
        .disabled(!opInfo.available || locked || viewModel.isBusy)
    }

    private func stepRow(_ step: StepState) -> some View {
        HStack {
            switch step.status {
            case .completed:
                Image(systemName: "checkmark.circle.fill").foregroundColor(.green)
            case .running:
                ProgressView()
            case .failed:
                Image(systemName: "xmark.circle.fill").foregroundColor(.red)
            case .pending:
                Image(systemName: "circle").foregroundColor(.secondary)
            }
            Text(NormalizeSelection.stepLabel(step.name, ops: viewModel.ops))
        }
    }

    private var cancelTitle: String {
        switch viewModel.phase {
        case .choose, .loading, .loadFailed, .starting: return "Cancel"
        case .review where viewModel.hasChanges: return "Discard"
        default: return "Close"
        }
    }

    @ToolbarContentBuilder
    private var toolbar: some ToolbarContent {
        ToolbarItem(placement: .cancellationAction) {
            Button(cancelTitle) {
                runTask?.cancel()
                isPresented = false
            }
            // Closing mid-save would leave the save running unseen.
            .disabled(viewModel.isBusy)
        }
        ToolbarItem(placement: .confirmationAction) {
            switch viewModel.phase {
            case .choose, .starting:
                Button("Run") {
                    runTask = Task { await viewModel.run() }
                }
                .disabled(!viewModel.canRun)
            case .review(_, _, let isAccepting, _) where viewModel.hasChanges:
                if isAccepting {
                    ProgressView().accessibilityLabel("Saving")
                } else {
                    Button("Accept") {
                        acceptTask = Task {
                            if await viewModel.accept() {
                                await onAccepted()
                                isPresented = false
                            }
                        }
                    }
                }
            default:
                EmptyView()
            }
        }
    }
}
