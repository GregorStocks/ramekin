import { createEffect, onCleanup, Show } from "solid-js";
import type { JSX } from "solid-js";

interface ModalProps {
  isOpen: () => boolean;
  onClose: () => void;
  title: string;
  children: JSX.Element;
  actions?: JSX.Element;
}

export default function Modal(props: ModalProps) {
  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Escape" && props.isOpen()) {
      props.onClose();
    }
  };

  const handleBackdropClick = (e: MouseEvent) => {
    if (e.target === e.currentTarget) {
      props.onClose();
    }
  };

  createEffect(() => {
    if (props.isOpen()) {
      document.addEventListener("keydown", handleKeyDown);
      document.body.style.overflow = "hidden";
    } else {
      document.removeEventListener("keydown", handleKeyDown);
      document.body.style.overflow = "";
    }
  });

  onCleanup(() => {
    document.removeEventListener("keydown", handleKeyDown);
    document.body.style.overflow = "";
  });

  return (
    <Show when={props.isOpen()}>
      <div class="modal-backdrop" onClick={handleBackdropClick}>
        <div class="modal-content">
          <div class="modal-header">
            <h3>{props.title}</h3>
          </div>
          <div class="modal-body">{props.children}</div>
          <Show when={props.actions}>
            <div class="modal-actions">{props.actions}</div>
          </Show>
        </div>
      </div>
    </Show>
  );
}
