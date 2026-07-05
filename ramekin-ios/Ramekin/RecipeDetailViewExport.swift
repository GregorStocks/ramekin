import SwiftUI

extension RecipeDetailView {
    @ViewBuilder
    func exportMenu(for recipe: RecipeResponse) -> some View {
        Menu {
            Button {
                Task { await viewModel.exportRecipeAsPaprika() }
            } label: {
                Label("Export as Paprika", systemImage: "doc.zipper")
            }
            .disabled(viewModel.isExporting)
            Button {
                Task { await viewModel.exportRecipeAsPDF(recipe) }
            } label: {
                Label("Export as PDF", systemImage: "doc.richtext")
            }
            .disabled(viewModel.isExporting)
        } label: {
            Label("Export", systemImage: "square.and.arrow.up")
        }
    }
}
