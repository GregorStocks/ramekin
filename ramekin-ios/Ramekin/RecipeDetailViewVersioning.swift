import SwiftUI

extension RecipeDetailView {
    func errorView(message: String) -> some View {
        VStack(spacing: 16) {
            Image(systemName: "exclamationmark.triangle")
                .font(.largeTitle)
                .foregroundColor(.orange)
            Text(message)
                .foregroundColor(.secondary)
                .multilineTextAlignment(.center)
            Button("Retry") {
                Task { await loadRecipe() }
            }
            .buttonStyle(.borderedProminent)
        }
        .padding()
        .frame(maxWidth: .infinity)
    }

    func inlineErrorBanner(message: String) -> some View {
        HStack(alignment: .top, spacing: 10) {
            Image(systemName: "exclamationmark.triangle.fill")
                .foregroundColor(.orange)
            Text(message)
                .font(.subheadline)
                .foregroundColor(.primary)
            Spacer()
        }
        .padding(12)
        .background(Color.orange.opacity(0.14))
        .clipShape(RoundedRectangle(cornerRadius: 12))
    }

    var versionHistorySection: some View {
        DisclosureGroup(isExpanded: $isVersionHistoryExpanded) {
            VStack(alignment: .leading, spacing: 12) {
                if isLoadingVersions {
                    ProgressView("Loading versions...")
                }

                if let versionHistoryError {
                    VStack(alignment: .leading, spacing: 8) {
                        Text(versionHistoryError)
                            .foregroundColor(.red)
                        Button("Retry") {
                            Task { await loadVersionHistory(force: true) }
                        }
                        .buttonStyle(.bordered)
                    }
                }

                if !versionHistory.isEmpty {
                    HStack {
                        Text(compareSelectionMessage)
                            .font(.caption)
                            .foregroundColor(.secondary)

                        Spacer()

                        if canCompareSelectedVersions {
                            Button {
                                Task { await openCompareSheet() }
                            } label: {
                                Label("Compare", systemImage: "square.split.2x1")
                            }
                            .buttonStyle(.borderedProminent)
                            .font(.caption)
                            .accessibilityIdentifier("compare-versions-button")
                        }
                    }
                }

                if !isLoadingVersions && versionHistoryError == nil && versionHistory.isEmpty {
                    Text("No saved versions yet.")
                        .font(.subheadline)
                        .foregroundColor(.secondary)
                }

                ForEach(versionHistory) { version in
                    versionHistoryRow(version)
                }
            }
            .padding(.top, 8)
        } label: {
            HStack {
                Text("Version History")
                    .font(.title3)
                    .fontWeight(.bold)

                if !versionHistory.isEmpty {
                    Text("(\(versionHistory.count))")
                        .font(.subheadline)
                        .foregroundColor(.secondary)
                }
            }
        }
        .accessibilityIdentifier("version-history-section")
    }

    func historicalVersionBanner(_ recipe: RecipeResponse) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .top, spacing: 10) {
                Image(systemName: "clock.arrow.circlepath")
                    .foregroundColor(.orange)
                VStack(alignment: .leading, spacing: 6) {
                    Text("Viewing version from \(formatDate(recipe.updatedAt))")
                        .font(.subheadline)
                        .fontWeight(.semibold)
                    Text(
                        "Switch back to the current recipe or revert this version to make it current again."
                    )
                    .font(.footnote)
                    .foregroundColor(.secondary)
                }
            }

            HStack(spacing: 12) {
                Button("View Current") {
                    Task { await loadRecipe() }
                }
                .buttonStyle(.bordered)
                .disabled(isLoading || isReverting)

                Button("Revert to This Version") {
                    revertCandidate = VersionSummary(
                        createdAt: recipe.updatedAt,
                        id: recipe.versionId,
                        isCurrent: false,
                        title: recipe.title,
                        versionSource: recipe.versionSource
                    )
                }
                .buttonStyle(.borderedProminent)
                .disabled(isLoading || isReverting)
            }
        }
        .padding(12)
        .background(Color.orange.opacity(0.14))
        .clipShape(RoundedRectangle(cornerRadius: 12))
    }

    func versionHistoryRow(_ version: VersionSummary) -> some View {
        let isDisplayedVersion = recipe?.versionId == version.id
        let isSelectedForCompare = compareSelection.contains(version.id)

        return VStack(alignment: .leading, spacing: 10) {
            HStack(alignment: .top, spacing: 12) {
                Button {
                    compareSelection = RecipeVersionSupport.toggleCompareSelection(
                        compareSelection,
                        versionId: version.id
                    )
                } label: {
                    Image(systemName: isSelectedForCompare ? "checkmark.circle.fill" : "circle")
                        .font(.title3)
                        .foregroundColor(isSelectedForCompare ? .orange : .secondary)
                }
                .buttonStyle(.plain)
                .disabled(isLoading || isLoadingCompare || isReverting)
                .accessibilityIdentifier("version-select-\(version.id.uuidString)")

                VStack(alignment: .leading, spacing: 8) {
                    HStack(spacing: 8) {
                        Text(formatDate(version.createdAt))
                            .font(.caption)
                            .foregroundColor(.secondary)

                        VersionSourceBadgeView(source: version.versionSource)

                        if version.isCurrent {
                            statusPill(label: "Current", color: .blue)
                        }

                        if isDisplayedVersion {
                            statusPill(label: "Viewing", color: .orange)
                        }
                    }

                    Text(version.title)
                        .font(.subheadline)
                        .fontWeight(.semibold)

                    HStack(spacing: 12) {
                        Button(version.isCurrent ? "View Current" : "View") {
                            Task { await displayVersion(version) }
                        }
                        .buttonStyle(.bordered)
                        .disabled(isDisplayedVersion || isLoading || isReverting)

                        if !version.isCurrent {
                            Button("Revert") {
                                revertCandidate = version
                            }
                            .buttonStyle(.borderedProminent)
                            .disabled(isLoading || isReverting)
                        }
                    }
                    .font(.caption)
                }
            }
        }
        .padding(12)
        .background(Color(.systemGray6))
        .clipShape(RoundedRectangle(cornerRadius: 12))
    }

    func statusPill(label: String, color: Color) -> some View {
        Text(label)
            .font(.caption2)
            .fontWeight(.semibold)
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(color.opacity(0.14))
            .foregroundColor(color)
            .clipShape(Capsule())
    }

    func formatDate(_ date: Date) -> String {
        let formatter = DateFormatter()
        formatter.dateStyle = .medium
        formatter.timeStyle = .short
        return formatter.string(from: date)
    }

    func sourceLinkSection(url: URL, name: String?) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Link(destination: url) {
                HStack {
                    Image(systemName: "link")
                    Text(name ?? url.host ?? "View Original")
                    Spacer()
                    Image(systemName: "arrow.up.right.square")
                }
                .foregroundColor(.orange)
            }

            Button {
                rescrapeTask?.cancel()
                rescrapeTask = Task {
                    await rescrapeRecipe()
                    await MainActor.run {
                        rescrapeTask = nil
                    }
                }
            } label: {
                Label(isRescraping ? "Rescraping..." : "Rescrape", systemImage: "arrow.clockwise")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.bordered)
            .disabled(isRescraping || actionsDisabledForHistoricalVersion)
        }
    }

    private var compareSelectionMessage: String {
        if compareSelection.isEmpty {
            return "Select two versions to compare."
        }

        return "\(compareSelection.count) selected for comparison."
    }
}

extension RecipeDetailView {
    func groupIngredientsBySection(_ ingredients: [Ingredient]) -> [(section: String?, items: [Ingredient])] {
        var groups: [(section: String?, items: [Ingredient])] = []
        var currentSection: String?
        var currentItems: [Ingredient] = []

        for ingredient in ingredients {
            if ingredient.section != currentSection {
                if !currentItems.isEmpty {
                    groups.append((section: currentSection, items: currentItems))
                }
                currentSection = ingredient.section
                currentItems = [ingredient]
            } else {
                currentItems.append(ingredient)
            }
        }

        if !currentItems.isEmpty {
            groups.append((section: currentSection, items: currentItems))
        }

        return groups
    }

    func formatIngredient(_ ingredient: Ingredient) -> String {
        var parts: [String] = []

        if let measurement = ingredient.measurements.first {
            if let amount = measurement.amount, !amount.isEmpty {
                parts.append(amount)
            }
            if let unit = measurement.unit, !unit.isEmpty {
                parts.append(unit)
            }
        }

        if ingredient.measurements.count > 1 {
            let altTexts = ingredient.measurements.dropFirst().compactMap { alt -> String? in
                let altParts = [alt.amount, alt.unit].compactMap { $0 }.filter { !$0.isEmpty }
                return altParts.isEmpty ? nil : altParts.joined(separator: " ")
            }
            if !altTexts.isEmpty {
                parts.append("(\(altTexts.joined(separator: ", ")))")
            }
        }

        parts.append(ingredient.item)

        return parts.joined(separator: " ")
    }

    @MainActor
    func loadRecipe(versionId: UUID? = nil) async {
        isLoading = true
        error = nil

        do {
            let loaded = try await RecipesAPI.getRecipe(id: recipeId, versionId: versionId)
            recipe = loaded
            isLoading = false

            if versionId == nil {
                currentVersionId = loaded.versionId
            } else if currentVersionId == nil {
                let current = try await RecipesAPI.getRecipe(id: recipeId)
                currentVersionId = current.versionId
            }

            if RecipeVersionSupport.shouldRefreshVersionHistory(
                requestedVersionId: versionId,
                isVersionHistoryExpanded: isVersionHistoryExpanded,
                hasCachedVersionHistory: !versionHistory.isEmpty
            ) {
                await loadVersionHistory(force: true)
            }
        } catch is CancellationError {
            isLoading = false
        } catch {
            self.error = error.localizedDescription
            isLoading = false
        }
    }

    @MainActor
    func loadVersionHistory(force: Bool = false) async {
        if isLoadingVersions {
            return
        }

        if !force && !versionHistory.isEmpty {
            return
        }

        isLoadingVersions = true
        versionHistoryError = nil

        do {
            let response = try await RecipesAPI.listVersions(id: recipeId)
            versionHistory = response.versions

            if let current = response.versions.first(where: { $0.isCurrent }) {
                currentVersionId = current.id
            }
        } catch is CancellationError {
        } catch {
            versionHistoryError = error.localizedDescription
        }

        isLoadingVersions = false
    }

    @MainActor
    func displayVersion(_ version: VersionSummary) async {
        if version.isCurrent {
            await loadRecipe()
        } else {
            await loadRecipe(versionId: version.id)
        }
    }

    @MainActor
    func openCompareSheet() async {
        guard canCompareSelectedVersions else {
            return
        }

        showingCompareSheet = true
        isLoadingCompare = true
        compareError = nil
        comparedOlderVersion = nil
        comparedNewerVersion = nil

        do {
            async let firstRecipe = RecipesAPI.getRecipe(
                id: recipeId,
                versionId: compareSelection[0]
            )
            async let secondRecipe = RecipesAPI.getRecipe(
                id: recipeId,
                versionId: compareSelection[1]
            )

            let first = try await firstRecipe
            let second = try await secondRecipe
            let orderedVersions = RecipeVersionSupport.sortForCompare(first, second)
            comparedOlderVersion = orderedVersions.older
            comparedNewerVersion = orderedVersions.newer
        } catch is CancellationError {
        } catch {
            compareError = "Failed to load versions for comparison"
        }

        isLoadingCompare = false
    }

    func closeCompareSheet() {
        showingCompareSheet = false
        isLoadingCompare = false
        compareError = nil
        comparedOlderVersion = nil
        comparedNewerVersion = nil
    }

    @MainActor
    func revert(to version: VersionSummary) async {
        revertCandidate = nil
        isReverting = true
        error = nil

        do {
            let historicalRecipe = try await RecipesAPI.getRecipe(
                id: recipeId,
                versionId: version.id
            )
            try await RecipeVersionSupport.revertRecipe(id: recipeId, from: historicalRecipe)

            await loadRecipe()
        } catch is CancellationError {
        } catch {
            self.error = "Failed to revert to this version"
        }

        isReverting = false
    }

    @MainActor
    func applyEnrichment(_ modified: RecipeContent) async {
        let updateRequest = UpdateRecipeRequest(
            cookTime: modified.cookTime,
            description: modified.description,
            difficulty: modified.difficulty,
            ingredients: modified.ingredients,
            instructions: modified.instructions,
            notes: modified.notes,
            nutritionalInfo: modified.nutritionalInfo,
            prepTime: modified.prepTime,
            rating: modified.rating,
            servings: modified.servings,
            sourceName: modified.sourceName,
            sourceUrl: modified.sourceUrl,
            tags: modified.tags,
            title: modified.title,
            totalTime: modified.totalTime
        )

        do {
            try await RecipesAPI.updateRecipe(id: recipeId, updateRecipeRequest: updateRequest)
            enrichResult = nil
            await loadRecipe()
        } catch is CancellationError {
        } catch {
            self.error = error.localizedDescription
        }
    }
}
