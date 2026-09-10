import { createUniqueId, onCleanup, onMount, Show } from "solid-js";
import type { JSX } from "solid-js";

interface ModalProps {
  isOpen: () => boolean;
  onClose: () => void;
  title: string;
  children: JSX.Element;
  actions?: JSX.Element;
}

export default function Modal(props: ModalProps) {
  return (
    <Show when={props.isOpen()}>
      <OpenModal {...props} />
    </Show>
  );
}

const openDialogs = new Set<HTMLDialogElement>();
let previousBodyOverflow = "";

function OpenModal(props: ModalProps) {
  let dialog!: HTMLDialogElement;
  let heading!: HTMLHeadingElement;
  const titleId = createUniqueId();

  onMount(() => {
    if (openDialogs.size === 0) {
      previousBodyOverflow = document.body.style.overflow;
      document.body.style.overflow = "hidden";
    }
    openDialogs.add(dialog);
    // Native modality makes the background inert and restores focus on close.
    dialog.showModal();

    // Async actions can disable or remove the focused control, including all
    // controls in a success state. Keep focus on the remaining dialog content.
    const observer = new MutationObserver(() => {
      if (
        !dialog.contains(document.activeElement) ||
        document.activeElement === dialog ||
        document.activeElement?.matches(":disabled")
      ) {
        heading.focus();
      }
    });
    observer.observe(dialog, {
      childList: true,
      subtree: true,
      attributes: true,
      attributeFilter: ["disabled", "hidden"],
    });

    onCleanup(() => {
      observer.disconnect();
      dialog.close();
      openDialogs.delete(dialog);
      if (openDialogs.size === 0) {
        document.body.style.overflow = previousBodyOverflow;
      }
    });
  });

  const handleKeyDown = (event: KeyboardEvent) => {
    if (event.key !== "Tab" || event.defaultPrevented) return;

    const controls = Array.from(
      dialog.querySelectorAll<HTMLElement>("*"),
    ).filter(
      (element) =>
        element.tabIndex >= 0 &&
        !element.matches(":disabled") &&
        element.getClientRects().length > 0 &&
        getComputedStyle(element).visibility === "visible",
    );
    const index = controls.indexOf(document.activeElement as HTMLElement);
    if (controls.length === 0) {
      event.preventDefault();
      heading.focus();
    } else if (event.shiftKey && index <= 0) {
      event.preventDefault();
      controls[controls.length - 1].focus();
    } else if (
      !event.shiftKey &&
      (index < 0 || index === controls.length - 1)
    ) {
      event.preventDefault();
      controls[0].focus();
    }
  };

  return (
    <dialog
      ref={dialog}
      class="modal-backdrop"
      aria-modal="true"
      aria-labelledby={titleId}
      onKeyDown={handleKeyDown}
      onCancel={(event) => {
        event.preventDefault();
        props.onClose();
      }}
      onClick={(event) => {
        if (event.target === event.currentTarget) props.onClose();
      }}
    >
      <div class="modal-content">
        <div class="modal-header">
          <h3 id={titleId} ref={heading} tabIndex={-1} autofocus>
            {props.title}
          </h3>
        </div>
        <div class="modal-body">{props.children}</div>
        <Show when={props.actions}>
          <div class="modal-actions">{props.actions}</div>
        </Show>
      </div>
    </dialog>
  );
}
