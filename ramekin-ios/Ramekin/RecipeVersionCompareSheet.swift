import SwiftUI

struct RecipeVersionCompareSheet: View {
    let olderVersion: RecipeResponse?
    let newerVersion: RecipeResponse?
    let isLoading: Bool
    let error: String?
    let onClose: () -> Void

    var body: some View {
        NavigationStack {
            ScrollView {
                content
                    .padding()
            }
            .navigationTitle("Compare Versions")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Close", action: onClose)
                }
            }
        }
    }

    @ViewBuilder
    private var content: some View {
        if isLoading {
            ProgressView("Loading versions...")
                .frame(maxWidth: .infinity, alignment: .center)
                .padding(.top, 40)
        } else if let error {
            Text(error)
                .foregroundColor(.red)
                .frame(maxWidth: .infinity, alignment: .center)
                .padding(.top, 40)
        } else if let olderVersion, let newerVersion {
            loadedContent(olderVersion: olderVersion, newerVersion: newerVersion)
        }
    }

    private func loadedContent(
        olderVersion: RecipeResponse,
        newerVersion: RecipeResponse
    ) -> some View {
        let versionDiffs = diffs(olderVersion: olderVersion, newerVersion: newerVersion)

        return VStack(alignment: .leading, spacing: 16) {
            compareHeader(olderVersion: olderVersion, newerVersion: newerVersion)

            if versionDiffs.isEmpty {
                Text("These versions are identical.")
                    .foregroundColor(.secondary)
                    .frame(maxWidth: .infinity, alignment: .center)
                    .padding(.top, 40)
            } else {
                ForEach(versionDiffs) { diff in
                    fieldDiff(diff)
                }
            }
        }
    }

    private func compareHeader(
        olderVersion: RecipeResponse,
        newerVersion: RecipeResponse
    ) -> some View {
        HStack(alignment: .top, spacing: 12) {
            versionSummaryCard(
                title: "Before",
                date: olderVersion.updatedAt,
                source: olderVersion.versionSource
            )

            Image(systemName: "arrow.right")
                .foregroundColor(.secondary)
                .padding(.top, 18)

            versionSummaryCard(
                title: "After",
                date: newerVersion.updatedAt,
                source: newerVersion.versionSource
            )
        }
    }

    private func versionSummaryCard(
        title: String,
        date: Date,
        source: String
    ) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(title)
                .font(.caption)
                .foregroundColor(.secondary)

            Text(formatDate(date))
                .font(.subheadline)
                .fontWeight(.semibold)

            VersionSourceBadgeView(source: source)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(12)
        .background(Color(.systemGray6))
        .clipShape(RoundedRectangle(cornerRadius: 12))
    }

    private func fieldDiff(_ diff: VersionFieldDiff) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(diff.label)
                .font(.headline)

            VStack(alignment: .leading, spacing: 4) {
                HStack(alignment: .top) {
                    Text("Before:")
                        .font(.caption)
                        .fontWeight(.bold)
                        .foregroundColor(.red)
                        .frame(width: 50, alignment: .leading)
                    Text(diff.before.isEmpty ? "(empty)" : diff.before)
                        .font(.body)
                        .foregroundColor(diff.before.isEmpty ? .secondary : .primary)
                }

                HStack(alignment: .top) {
                    Text("After:")
                        .font(.caption)
                        .fontWeight(.bold)
                        .foregroundColor(.green)
                        .frame(width: 50, alignment: .leading)
                    Text(diff.after.isEmpty ? "(empty)" : diff.after)
                        .font(.body)
                        .foregroundColor(diff.after.isEmpty ? .secondary : .primary)
                }
            }
            .padding(12)
            .background(Color(.systemGray6))
            .clipShape(RoundedRectangle(cornerRadius: 8))
        }
    }

    private func diffs(
        olderVersion: RecipeResponse,
        newerVersion: RecipeResponse
    ) -> [VersionFieldDiff] {
        textDiffs(olderVersion: olderVersion, newerVersion: newerVersion)
            + metadataDiffs(olderVersion: olderVersion, newerVersion: newerVersion)
            + sourceDiffs(olderVersion: olderVersion, newerVersion: newerVersion)
    }

    private func textDiffs(
        olderVersion: RecipeResponse,
        newerVersion: RecipeResponse
    ) -> [VersionFieldDiff] {
        [
            makeDiff(label: "Title", before: olderVersion.title, after: newerVersion.title),
            makeDiff(
                label: "Description",
                before: olderVersion.description ?? "",
                after: newerVersion.description ?? ""
            ),
            makeDiff(
                label: "Ingredients",
                before: RecipeVersionSupport.formatIngredients(olderVersion.ingredients),
                after: RecipeVersionSupport.formatIngredients(newerVersion.ingredients)
            ),
            makeDiff(
                label: "Instructions",
                before: olderVersion.instructions,
                after: newerVersion.instructions
            ),
            makeDiff(
                label: "Tags",
                before: RecipeVersionSupport.formatTags(olderVersion.tags),
                after: RecipeVersionSupport.formatTags(newerVersion.tags)
            ),
            makeDiff(label: "Notes", before: olderVersion.notes ?? "", after: newerVersion.notes ?? "")
        ]
        .compactMap { $0 }
    }

    private func metadataDiffs(
        olderVersion: RecipeResponse,
        newerVersion: RecipeResponse
    ) -> [VersionFieldDiff] {
        [
            makeDiff(
                label: "Prep Time",
                before: olderVersion.prepTime ?? "",
                after: newerVersion.prepTime ?? ""
            ),
            makeDiff(
                label: "Cook Time",
                before: olderVersion.cookTime ?? "",
                after: newerVersion.cookTime ?? ""
            ),
            makeDiff(
                label: "Total Time",
                before: olderVersion.totalTime ?? "",
                after: newerVersion.totalTime ?? ""
            ),
            makeDiff(
                label: "Servings",
                before: olderVersion.servings ?? "",
                after: newerVersion.servings ?? ""
            ),
            makeDiff(
                label: "Difficulty",
                before: olderVersion.difficulty ?? "",
                after: newerVersion.difficulty ?? ""
            ),
            makeDiff(
                label: "Nutritional Info",
                before: olderVersion.nutritionalInfo ?? "",
                after: newerVersion.nutritionalInfo ?? ""
            )
        ]
        .compactMap { $0 }
    }

    private func sourceDiffs(
        olderVersion: RecipeResponse,
        newerVersion: RecipeResponse
    ) -> [VersionFieldDiff] {
        [
            makeDiff(
                label: "Source Name",
                before: olderVersion.sourceName ?? "",
                after: newerVersion.sourceName ?? ""
            ),
            makeDiff(
                label: "Source URL",
                before: olderVersion.sourceUrl ?? "",
                after: newerVersion.sourceUrl ?? ""
            )
        ]
        .compactMap { $0 }
    }

    private func makeDiff(
        label: String,
        before: String,
        after: String
    ) -> VersionFieldDiff? {
        guard before != after else {
            return nil
        }

        return VersionFieldDiff(label: label, before: before, after: after)
    }

    private func formatDate(_ date: Date) -> String {
        let formatter = DateFormatter()
        formatter.dateStyle = .medium
        formatter.timeStyle = .short
        return formatter.string(from: date)
    }
}

private struct VersionFieldDiff: Identifiable {
    let label: String
    let before: String
    let after: String

    var id: String {
        label
    }
}

struct VersionSourceBadgeView: View {
    let source: String

    var body: some View {
        Text(RecipeVersionSupport.sourceLabel(for: source))
            .font(.caption)
            .fontWeight(.semibold)
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(backgroundColor)
            .foregroundColor(foregroundColor)
            .clipShape(Capsule())
    }

    private var foregroundColor: Color {
        switch source {
        case "user":
            return .blue
        case "scrape":
            return .orange
        case "enrich", "enrichment":
            return .green
        default:
            return .secondary
        }
    }

    private var backgroundColor: Color {
        foregroundColor.opacity(0.14)
    }
}
