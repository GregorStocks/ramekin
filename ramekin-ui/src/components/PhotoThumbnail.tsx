import { createSignal, Show, onMount, onCleanup } from "solid-js";

interface PhotoThumbnailProps {
  photoId: string;
  token: string;
  onRemove?: () => void;
  alt?: string;
  class?: string;
}

export default function PhotoThumbnail(props: PhotoThumbnailProps) {
  const [src, setSrc] = createSignal<string | null>(null);
  const [failed, setFailed] = createSignal(false);

  onMount(async () => {
    try {
      const response = await fetch(`/api/photos/${props.photoId}`, {
        headers: { Authorization: `Bearer ${props.token}` },
      });
      if (response.ok) {
        const blob = await response.blob();
        setSrc(URL.createObjectURL(blob));
      } else {
        setFailed(true);
      }
    } catch {
      setFailed(true);
    }
  });

  onCleanup(() => {
    const url = src();
    if (url) URL.revokeObjectURL(url);
  });

  return (
    <div class={props.class ?? "photo-thumbnail"}>
      <Show when={failed()}>
        <div class="photo-error">Failed to load</div>
      </Show>
      <Show when={!failed() && src()}>
        <img src={src()!} alt={props.alt ?? "Recipe photo"} />
      </Show>
      <Show when={!failed() && !src()}>
        <div class="photo-loading">Loading...</div>
      </Show>
      <Show when={props.onRemove}>
        <button type="button" class="photo-remove" onClick={props.onRemove}>
          &times;
        </button>
      </Show>
    </div>
  );
}
