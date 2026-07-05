import SwiftUI

extension RecipeDetailView {
    static let versionHistoryDateFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateStyle = .medium
        formatter.timeStyle = .short
        formatter.calendar = .autoupdatingCurrent
        formatter.locale = .autoupdatingCurrent
        formatter.timeZone = .autoupdatingCurrent
        return formatter
    }()

    func errorView(message: String) -> some View {
        VStack(spacing: 16) {
            Image(systemName: "exclamationmark.triangle")
                .font(.largeTitle)
                .foregroundColor(.orange)
            Text(message)
                .foregroundColor(.secondary)
                .multilineTextAlignment(.center)
            Button("Retry") {
                Task { await viewModel.loadRecipe() }
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
        DisclosureGroup(isExpanded: $viewModel.isVersionHistoryExpanded) {
            VStack(alignment: .leading, spacing: 12) {
                if viewModel.isLoadingVersions {
                    ProgressView("Loading versions...")
                }

                if let versionHistoryError = viewModel.versionHistoryError {
                    VStack(alignment: .leading, spacing: 8) {
                        Text(versionHistoryError)
                            .foregroundColor(.red)
                        Button("Retry") {
                            Task { await viewModel.loadVersionHistory(force: true) }
                        }
                        .buttonStyle(.bordered)
                    }
                }

                if !viewModel.versionHistory.isEmpty {
                    HStack {
                        Text(compareSelectionMessage)
                            .font(.caption)
                            .foregroundColor(.secondary)

                        Spacer()

                        if canCompareSelectedVersions {
                            Button {
                                Task { await viewModel.openCompareSheet() }
                            } label: {
                                Label("Compare", systemImage: "square.split.2x1")
                            }
                            .buttonStyle(.borderedProminent)
                            .font(.caption)
                            .accessibilityIdentifier("compare-versions-button")
                        }
                    }
                }

                if !viewModel.isLoadingVersions
                    && viewModel.versionHistoryError == nil
                    && viewModel.versionHistory.isEmpty {
                    Text("No saved versions yet.")
                        .font(.subheadline)
                        .foregroundColor(.secondary)
                }

                ForEach(viewModel.versionHistory) { version in
                    versionHistoryRow(version)
                }
            }
            .padding(.top, 8)
        } label: {
            HStack {
                Text("Version History")
                    .font(.title3)
                    .fontWeight(.bold)

                if !viewModel.versionHistory.isEmpty {
                    Text("(\(viewModel.versionHistory.count))")
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
                    Task { await viewModel.loadRecipe() }
                }
                .buttonStyle(.bordered)
                .disabled(viewModel.isLoading || viewModel.isReverting)

                Button("Revert to This Version") {
                    viewModel.revertCandidate = VersionSummary(
                        createdAt: recipe.updatedAt,
                        id: recipe.versionId,
                        isCurrent: false,
                        title: recipe.title,
                        versionSource: recipe.versionSource
                    )
                }
                .buttonStyle(.borderedProminent)
                .disabled(viewModel.isLoading || viewModel.isReverting)
            }
        }
        .padding(12)
        .background(Color.orange.opacity(0.14))
        .clipShape(RoundedRectangle(cornerRadius: 12))
    }

    func versionHistoryRow(_ version: VersionSummary) -> some View {
        let isDisplayedVersion = viewModel.recipe?.versionId == version.id
        let isSelectedForCompare = viewModel.compareSelection.contains(version.id)

        return VStack(alignment: .leading, spacing: 10) {
            HStack(alignment: .top, spacing: 12) {
                Button {
                    viewModel.toggleCompareSelection(versionId: version.id)
                } label: {
                    Image(systemName: isSelectedForCompare ? "checkmark.circle.fill" : "circle")
                        .font(.title3)
                        .foregroundColor(isSelectedForCompare ? .orange : .secondary)
                }
                .buttonStyle(.plain)
                .disabled(viewModel.isLoading || viewModel.isLoadingCompare || viewModel.isReverting)
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
                            Task { await viewModel.displayVersion(version) }
                        }
                        .buttonStyle(.bordered)
                        .disabled(isDisplayedVersion || viewModel.isLoading || viewModel.isReverting)

                        if !version.isCurrent {
                            Button("Revert") {
                                viewModel.revertCandidate = version
                            }
                            .buttonStyle(.borderedProminent)
                            .disabled(viewModel.isLoading || viewModel.isReverting)
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
        Self.versionHistoryDateFormatter.string(from: date)
    }

    func sourceLinkSection(url: URL, name: String?) -> some View {
        Link(destination: url) {
            HStack {
                Image(systemName: "link")
                Text(name ?? url.host ?? "View Original")
                Spacer()
                Image(systemName: "arrow.up.right.square")
            }
            .foregroundColor(.orange)
        }
    }

    private var compareSelectionMessage: String {
        if viewModel.compareSelection.isEmpty {
            return "Select two versions to compare."
        }

        return "\(viewModel.compareSelection.count) selected for comparison."
    }
}

extension RecipeDetailView {
    func groupIngredientsBySection(_ ingredients: [Ingredient]) -> [(section: String?, items: [Ingredient])] {
        groupConsecutiveItemsBySection(ingredients) { $0.section }
    }
}
