import SwiftUI

struct MealPlanItemCard: View {
    let meal: MealPlanItem
    let onEdit: () -> Void
    let onDelete: () -> Void

    var body: some View {
        HStack(spacing: 10) {
            NavigationLink(value: NavigationDestination.recipe(meal.recipeId)) {
                HStack(spacing: 10) {
                    RecipeThumbnail(photoId: meal.thumbnailPhotoId, size: 44)
                        // Beside the title it illustrates; read aloud it only adds "photo".
                        .accessibilityHidden(true)

                    VStack(alignment: .leading, spacing: 2) {
                        Text(meal.recipeTitle)
                            .font(.subheadline)
                            .foregroundColor(.primary)
                            .lineLimit(2)

                        if let notes = meal.notes, !notes.isEmpty {
                            Text(notes)
                                .font(.caption)
                                .foregroundColor(.secondary)
                                .lineLimit(2)
                        }
                    }
                }
            }
            .buttonStyle(.plain)

            Spacer()

            Button(action: onEdit) {
                Image(systemName: "pencil.circle")
                    .foregroundColor(.secondary)
                    .font(.body)
                    .frame(minWidth: 44, minHeight: 44)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Edit \(meal.recipeTitle)")

            Button(action: onDelete) {
                Image(systemName: "xmark.circle.fill")
                    .foregroundColor(.secondary)
                    .font(.body)
                    .frame(minWidth: 44, minHeight: 44)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Remove \(meal.recipeTitle)")
        }
        .padding(.vertical, 4)
        .padding(.horizontal, 8)
        .background(Color(.systemGray6))
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }
}
