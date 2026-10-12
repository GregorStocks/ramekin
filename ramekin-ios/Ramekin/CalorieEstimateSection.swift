import SwiftUI

/// Waits between calorie-estimate polls while names are still being
/// recognized: doubling from 2 s to 30 s, giving up once the waits would pass
/// 5 minutes. A stalled resolver (a provider outage, the worker backing off)
/// then costs a few requests, not one every 2 s forever. Pinned with web by
/// shared-test-vectors/calorie-poll-delays.json.
enum CaloriePolling {
    private static let firstDelaySeconds = 2
    private static let maxDelaySeconds = 30
    private static let totalSeconds = 300

    /// Seconds to wait before poll `index` (0-based), or nil to stop.
    static func delaySeconds(beforePoll index: Int) -> Int? {
        guard index >= 0 else { return nil }
        var waited = 0
        var delay = firstDelaySeconds
        for poll in 0...index {
            waited += delay
            if waited > totalSeconds { return nil }
            if poll == index { return delay }
            delay = min(delay * 2, maxDelaySeconds)
        }
        return nil
    }
}

@MainActor
final class CalorieEstimateViewModel: ObservableObject {
    @Published private(set) var response: CalorieEstimateResponse?
    @Published private(set) var request: EstimateCaloriesRequest?
    @Published private(set) var isLoading = false
    @Published private(set) var error: String?

    private let estimate: (EstimateCaloriesRequest) async throws -> CalorieEstimateResponse
    private let pollDelay: (Int) -> Duration?
    private var generation = UUID()

    init(
        pollDelay: @escaping (Int) -> Duration? = { index in
            CaloriePolling.delaySeconds(beforePoll: index).map { .seconds($0) }
        },
        estimate: @escaping (EstimateCaloriesRequest) async throws -> CalorieEstimateResponse = {
            try await RecipesAPI.estimateCalories(estimateCaloriesRequest: $0)
        }
    ) {
        self.pollDelay = pollDelay
        self.estimate = estimate
    }

    func load(_ request: EstimateCaloriesRequest) async {
        let generation = UUID()
        self.generation = generation
        self.request = request
        response = nil
        error = nil
        isLoading = true
        let result: CalorieEstimateResponse
        do {
            result = try await estimate(request)
        } catch is CancellationError {
            return
        } catch {
            guard self.generation == generation, !Task.isCancelled else { return }
            self.error = "Could not estimate calories: \(error.localizedDescription)"
            isLoading = false
            return
        }
        guard self.generation == generation, !Task.isCancelled else { return }
        response = result
        isLoading = false
        await poll(after: result, request: request, generation: generation)
    }

    /// Names still being recognized in the background: ask again quietly,
    /// backing off, until the estimate includes them. Ends when the schedule
    /// runs out, the view goes away (cancellation), or a newer request
    /// replaces this one.
    private func poll(
        after first: CalorieEstimateResponse,
        request: EstimateCaloriesRequest,
        generation: UUID
    ) async {
        var result = first
        var polls = 0
        while result.resolving, let delay = pollDelay(polls) {
            polls += 1
            do {
                try await Task.sleep(for: delay)
            } catch {
                return
            }
            guard self.generation == generation else { return }
            do {
                result = try await estimate(request)
            } catch {
                // A replaced or cancelled request's failure ends this
                // (URLError.cancelled is not a CancellationError). Any other
                // keeps showing the last good estimate; the next poll may
                // recover.
                guard self.generation == generation, !Task.isCancelled else { return }
                continue
            }
            guard self.generation == generation, !Task.isCancelled else { return }
            // Most polls return the same estimate. Publishing it anyway
            // re-renders the recipe screen every poll, and doing that while
            // the toolbar menu is open kept the app's main thread busy for
            // minutes on CI.
            if result != response {
                response = result
            }
        }
    }
}

struct CalorieEstimateSection: View {
    let request: EstimateCaloriesRequest
    @StateObject private var model = CalorieEstimateViewModel()

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Estimated Calories")
                .font(.headline)
                .accessibilityAddTraits(.isHeader)
            if model.request != request || model.isLoading {
                ProgressView("Calculating calories…")
            } else if let error = model.error {
                Text(error)
                    .foregroundStyle(.red)
                Button("Retry") {
                    Task { await model.load(request) }
                }
            } else if let response = model.response {
                Text(response.headline)
                    .font(.title3.weight(.semibold))
                if let secondary = response.secondary {
                    Text(secondary)
                        .foregroundStyle(.secondary)
                }
                if !response.notCounted.isEmpty {
                    Text("Not counted: \(response.notCounted.joined(separator: ", "))")
                        .foregroundStyle(.secondary)
                }
                if !response.lines.isEmpty {
                    DisclosureGroup("How is this calculated?") {
                        VStack(alignment: .leading, spacing: 6) {
                            ForEach(response.lines, id: \.index) { line in
                                HStack(alignment: .firstTextBaseline) {
                                    Text(line.item)
                                    Spacer()
                                    Text(line.text)
                                        .foregroundStyle(.secondary)
                                }
                            }
                            Text("Based on USDA reference foods and the listed ingredient amounts, with model estimates where noted.")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                        .padding(.top, 4)
                    }
                    .accessibilityIdentifier("calorieBreakdown")
                }
            }
        }
        .accessibilityIdentifier("calorieEstimate")
        .task(id: request) {
            await model.load(request)
        }
    }
}
