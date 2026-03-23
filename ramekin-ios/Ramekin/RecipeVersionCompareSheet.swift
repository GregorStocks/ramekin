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
                VStack(alignment: .leading, spacing: 16) {
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
                        compareHeader(olderVersion: olderVersion, newerVersion: newerVersion)

                        if diffs(olderVersion: olderVersion, newerVersion: newerVersion).isEmpty {
                            Text("These versions are identical.")
                                .foregroundColor(.secondary)
                                .frame(maxWidth: .infinity, alignment: .center)
                                .padding(.top, 40)
                        } else {
                            ForEach(diffs(olderVersion: olderVersion, newerVersion: newerVersion)) { diff in
                                fieldDiff(diff)
                            }
                        }
                    }
                }
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
        let candidates = [
            ("Title", olderVersion.title, newerVersion.title),
            ("Description", olderVersion.description ?? "", newerVersion.description ?? ""),
            (
                "Ingredients",
                RecipeVersionSupport.formatIngredients(olderVersion.ingredients),
                RecipeVersionSupport.formatIngredients(newerVersion.ingredients)
            ),
            ("Instructions", olderVersion.instructions, newerVersion.instructions),
            (
                "Tags",
                RecipeVersionSupport.formatTags(olderVersion.tags),
                RecipeVersionSupport.formatTags(newerVersion.tags)
            ),
            ("Notes", olderVersion.notes ?? "", newerVersion.notes ?? ""),
            ("Prep Time", olderVersion.prepTime ?? "", newerVersion.prepTime ?? ""),
            ("Cook Time", olderVersion.cookTime ?? "", newerVersion.cookTime ?? ""),
            ("Total Time", olderVersion.totalTime ?? "", newerVersion.totalTime ?? ""),
            ("Servings", olderVersion.servings ?? "", newerVersion.servings ?? ""),
            ("Difficulty", olderVersion.difficulty ?? "", newerVersion.difficulty ?? ""),
            (
                "Nutritional Info",
                olderVersion.nutritionalInfo ?? "",
                newerVersion.nutritionalInfo ?? ""
            ),
            ("Source Name", olderVersion.sourceName ?? "", newerVersion.sourceName ?? ""),
            ("Source URL", olderVersion.sourceUrl ?? "", newerVersion.sourceUrl ?? "")
        ]

        return candidates.compactMap { label, before, after in
            makeDiff(label: label, before: before, after: after)
        }
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
