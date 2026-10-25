import Modal from "./Modal";
import RecipeContentDiff from "./RecipeContentDiff";
import type { RecipeContent } from "ramekin-client";

interface EnrichPreviewModalProps {
  isOpen: () => boolean;
  onClose: () => void;
  currentContent: RecipeContent;
  enrichedContent: RecipeContent;
  onApply: () => void;
  applying: boolean;
}

/** Reviews an Edit with AI result before it's saved. */
export default function EnrichPreviewModal(props: EnrichPreviewModalProps) {
  return (
    <Modal
      isOpen={props.isOpen}
      onClose={props.onClose}
      title="AI Edit Suggestions"
      actions={
        <>
          <button type="button" class="btn" onClick={props.onClose}>
            Cancel
          </button>
          <button
            type="button"
            class="btn btn-primary"
            onClick={props.onApply}
            disabled={props.applying}
          >
            {props.applying ? "Applying..." : "Apply Changes"}
          </button>
        </>
      }
    >
      <div class="enrich-preview">
        <RecipeContentDiff
          oldContent={props.currentContent}
          newContent={props.enrichedContent}
          emptyMessage="The AI suggested no changes."
        />
      </div>
    </Modal>
  );
}
