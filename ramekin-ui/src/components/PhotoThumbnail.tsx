import { createSignal, Show, onMount, onCleanup } from "solid-js";

interface PhotoThumbnailProps {
  photoId: string;
  token: string;
  onRemove?: () => void;
  alt?: string;
  class?: string;
  thumbnailSize?: number;
}

export default function PhotoThumbnail(props: PhotoThumbnailProps) {
  const [src, setSrc] = createSignal<string | null>(null);
  const [error, setError] = createSignal(false);

  onMount(async () => {
    try {
      const url = props.thumbnailSize
        ? `/api/photos/${props.photoId}/thumbnail?size=${props.thumbnailSize}`
        : `/api/photos/${props.photoId}`;
      const response = await fetch(url, {
        headers: { Authorization: `Bearer ${props.token}` },
      });
      if (response.ok) {
        const blob = await response.blob();
        setSrc(URL.createObjectURL(blob));
      } else {
        setError(true);
      }
    } catch {
      setError(true);
    }
  });

  onCleanup(() => {
    const url = src();
    if (url) URL.revokeObjectURL(url);
  });

  return (
    <div class={props.class ?? "photo-thumbnail"}>
      <Show
        when={src()}
        fallback={
          <div class="photo-loading">
            {error() ? "Failed to load" : "Loading..."}
          </div>
        }
      >
        <img src={src()!} alt={props.alt ?? "Recipe photo"} />
      </Show>
      <Show when={props.onRemove}>
        <button type="button" class="photo-remove" onClick={props.onRemove}>
          &times;
        </button>
      </Show>
    </div>
  );
}
