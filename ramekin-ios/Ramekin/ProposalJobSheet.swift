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
    enum Phase: Equatable {
        case loading
        case choose
        case running
        case review
        case failed(String)
    }

    let recipeId: UUID
    let mode: ProposalJobMode
    @Published var ops: [NormalizeOpInfo] = []
    @Published var instruction = ""
    @Published var selected: Set<String> = []
    @Published var phase: Phase = .loading
    @Published var steps: [StepState] = []
    @Published var proposal: NormalizeProposalResponse?
    @Published var error: String?
    @Published var isBusy = false
    private(set) var jobId: UUID?
    var pollIntervalNanoseconds: UInt64 = 1_000_000_000

    private let api: ProposalJobAPIClient

    init(recipeId: UUID, mode: ProposalJobMode, api: ProposalJobAPIClient = .live) {
        self.recipeId = recipeId
        self.mode = mode
        self.api = api
    }

    var canRun: Bool {
        switch mode {
        case .normalize:
            return !effectiveOps.isEmpty
        case .edit:
            return !instruction.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        }
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
        error = nil
        if case .edit = mode {
            phase = .choose
            return
        }
        phase = .loading
        do {
            let response = try await api.listOps(recipeId)
            ops = response.ops
            selected = NormalizeSelection.defaultSelection(response.ops)
        } catch is CancellationError {
            return
        } catch {
            self.error = APIErrorFormatter.userMessage(from: error, fallback: "Failed to load normalize ops")
        }
        phase = .choose
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
        isBusy = true
        error = nil
        let jobId: UUID
        do {
            jobId = try await startJob()
        } catch is CancellationError {
            isBusy = false
            return
        } catch {
            self.error = APIErrorFormatter.userMessage(from: error, fallback: "Failed to start")
            isBusy = false
            return
        }
        isBusy = false
        self.jobId = jobId
        steps = []
        phase = .running

        do {
            // AI can be slow, so there's no deadline; closing the sheet
            // cancels the task.
            while true {
                try Task.checkCancellation()
                let job = try await api.getScrape(jobId)
                steps = job.steps
                if job.status == "completed" {
                    proposal = try await api.getProposal(recipeId, jobId)
                    phase = .review
                    return
                }
                if job.status == "failed" {
                    phase = .failed(job.error ?? "The job failed")
                    return
                }
                try await Task.sleep(nanoseconds: pollIntervalNanoseconds)
            }
        } catch is CancellationError {
            return
        } catch {
            phase = .failed(APIErrorFormatter.userMessage(from: error, fallback: "Lost track of the job"))
        }
    }

    /// Saves the proposal as the recipe's new version; returns whether it did.
    func accept() async -> Bool {
        guard let jobId else { return false }
        isBusy = true
        error = nil
        defer { isBusy = false }
        do {
            _ = try await api.accept(recipeId, jobId)
            return true
        } catch is CancellationError {
            return false
        } catch {
            self.error = APIErrorFormatter.userMessage(from: error, fallback: "Failed to save the new version")
            return false
        }
    }
}

struct ProposalJobSheet: View {
    @StateObject private var viewModel: ProposalJobViewModel
    @Binding var isPresented: Bool
    /// The proposal was saved as the recipe's new version.
    let onAccepted: () async -> Void
    @State private var runTask: Task<Void, Never>?

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
                if let error = viewModel.error {
                    Section {
                        Text(error).foregroundColor(.red)
                    }
                }
                content
            }
            .navigationTitle(viewModel.mode == .normalize ? "Re-normalize" : "Edit with AI")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { toolbar }
            .interactiveDismissDisabled(viewModel.isBusy)
        }
        .task { await viewModel.load() }
        // Closing the sheet abandons the job; it proposes, so nothing is saved.
        .onDisappear { runTask?.cancel() }
    }

    @ViewBuilder
    private var content: some View {
        switch viewModel.phase {
        case .loading:
            Section {
                ProgressView().frame(maxWidth: .infinity)
            }
        case .choose where viewModel.mode != .normalize:
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
        case .choose:
            Section {
                Text("Source ops replace the recipe's content, hand edits included. You review the result before it's saved.")
                    .font(.footnote)
                    .foregroundColor(.secondary)
            }
            opSection(.source, title: "Source")
            opSection(.normalize, title: "Normalize")
        case .running:
            Section("Progress") {
                if viewModel.steps.isEmpty {
                    Text("Starting…")
                }
                ForEach(viewModel.steps, id: \.name) { step in
                    stepRow(step)
                }
            }
        case .review:
            if let summary = viewModel.proposal?.summary, !summary.isEmpty {
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
        case .failed(let message):
            Section {
                Text(message).foregroundColor(.red)
            }
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
            case "completed":
                Image(systemName: "checkmark.circle.fill").foregroundColor(.green)
            case "running":
                ProgressView()
            case "failed":
                Image(systemName: "xmark.circle.fill").foregroundColor(.red)
            case "skipped":
                Image(systemName: "minus.circle").foregroundColor(.secondary)
            default:
                Image(systemName: "circle").foregroundColor(.secondary)
            }
            Text(NormalizeSelection.stepLabel(step.name, ops: viewModel.ops))
        }
    }

    private var cancelTitle: String {
        switch viewModel.phase {
        case .choose, .loading: return "Cancel"
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
        }
        ToolbarItem(placement: .confirmationAction) {
            switch viewModel.phase {
            case .choose:
                Button("Run") {
                    runTask = Task { await viewModel.run() }
                }
                .disabled(!viewModel.canRun || viewModel.isBusy)
            case .review where viewModel.hasChanges:
                if viewModel.isBusy {
                    ProgressView().accessibilityLabel("Saving")
                } else {
                    Button("Accept") {
                        Task {
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
