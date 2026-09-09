import SwiftUI

@MainActor
final class CalorieEstimateViewModel: ObservableObject {
    @Published private(set) var response: CalorieEstimateResponse?
    @Published private(set) var request: EstimateCaloriesRequest?
    @Published private(set) var isLoading = false
    @Published private(set) var error: String?

    private let estimate: (EstimateCaloriesRequest) async throws -> CalorieEstimateResponse
    private var generation = UUID()

    init(estimate: @escaping (EstimateCaloriesRequest) async throws -> CalorieEstimateResponse = {
        try await RecipesAPI.estimateCalories(estimateCaloriesRequest: $0)
    }) {
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
            let result = try await estimate(request)
            guard self.generation == generation, !Task.isCancelled else { return }
            response = result
        } catch {
            guard self.generation == generation, !Task.isCancelled else { return }
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
                Text(response.summary)
                if let perServing = response.perServingSummary {
                    Text(perServing)
                }
                ForEach(response.unknownIngredients, id: \.index) { ingredient in
                    Text("\(ingredient.item): \(ingredient.reason)")
                }
                Text("Based on USDA reference foods and the listed ingredient amounts.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
        .accessibilityIdentifier("calorieEstimate")
        .task(id: request) {
            await model.load(request)
        }
    }
}
