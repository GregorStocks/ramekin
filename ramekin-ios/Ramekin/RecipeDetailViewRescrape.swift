import SwiftUI

extension RecipeDetailView {
    @MainActor
    func rescrapeFromSource() async {
        isRescraping = true
        rescrapeError = nil
        error = nil

        do {
            let response = try await RecipesAPI.rescrape(id: recipeId)
            let jobId = response.jobId

            let pollStartTime = Date()
            let timeoutInterval: TimeInterval = 120

            while true {
                try Task.checkCancellation()

                let job = try await ScrapeAPI.getScrape(id: jobId)

                if job.status == "completed" {
                    await loadRecipe()
                    isRescraping = false
                    return
                } else if job.status == "failed" {
                    rescrapeError = job.error ?? "Unknown error"
                    isRescraping = false
                    return
                }

                if Date().timeIntervalSince(pollStartTime) > timeoutInterval {
                    rescrapeError = "Rescrape timed out"
                    isRescraping = false
                    return
                }

                try await Task.sleep(nanoseconds: 500_000_000)
            }
        } catch is CancellationError {
            isRescraping = false
        } catch {
            rescrapeError = "Failed to rescrape recipe"
            isRescraping = false
        }
    }

    func rescrapeProgressBanner() -> some View {
        HStack(spacing: 10) {
            ProgressView()
            Text("Rescraping from source...")
                .font(.subheadline)
                .foregroundColor(.primary)
            Spacer()
        }
        .padding(12)
        .background(Color.orange.opacity(0.14))
        .clipShape(RoundedRectangle(cornerRadius: 12))
    }
}
