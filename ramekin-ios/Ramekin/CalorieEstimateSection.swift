import SwiftUI

@MainActor
final class CalorieEstimateViewModel: ObservableObject {
    @Published private(set) var response: CalorieEstimateResponse?
    @Published private(set) var request: EstimateCaloriesRequest?
    @Published private(set) var isLoading = false
    @Published private(set) var error: String?

    private let estimate: (EstimateCaloriesRequest) async throws -> CalorieEstimateResponse
    private let pollInterval: Duration
    private var generation = UUID()

    init(
        pollInterval: Duration = .seconds(2),
        estimate: @escaping (EstimateCaloriesRequest) async throws -> CalorieEstimateResponse = {
            try await RecipesAPI.estimateCalories(estimateCaloriesRequest: $0)
        }
    ) {
        self.pollInterval = pollInterval
        self.estimate = estimate
    }

    func load(_ request: EstimateCaloriesRequest) async {
        let generation = UUID()
        self.generation = generation
        self.request = request
        response = nil
        error = nil
        isLoading = true
        do {
            var result = try await estimate(request)
            guard self.generation == generation, !Task.isCancelled else { return }
            response = result
            isLoading = false
            // Names still being recognized in the background: ask again
            // quietly until the estimate includes them. Ends when the view
            // goes away (cancellation) or a newer request replaces this one.
            while result.resolving {
                try await Task.sleep(for: pollInterval)
                guard self.generation == generation else { return }
                result = try await estimate(request)
                guard self.generation == generation, !Task.isCancelled else { return }
                response = result
            }
        } catch is CancellationError {
            return
        } catch {
            guard self.generation == generation, !Task.isCancelled else { return }
            response = nil
            self.error = "Could not estimate calories: \(error.localizedDescription)"
        }
        isLoading = false
    }
}

struct CalorieEstimateSection: View {
    let request: EstimateCaloriesRequest
    @StateObject private var model = CalorieEstimateViewModel()

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Estimated calories")
                .font(.headline)
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
                            Text("Based on USDA reference foods and the listed ingredient amounts.")
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
