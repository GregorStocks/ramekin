import { createSignal, Show, onMount, onCleanup } from "solid-js";

interface PhotoThumbnailProps {
  photoId: string;
  token: string;
  onRemove: () => void;
}

export default function PhotoThumbnail(props: PhotoThumbnailProps) {
  const [src, setSrc] = createSignal<string | null>(null);

  onMount(async () => {
    const response = await fetch(`/api/photos/${props.photoId}`, {
      headers: { Authorization: `Bearer ${props.token}` },
    });
    if (response.ok) {
      const blob = await response.blob();
      setSrc(URL.createObjectURL(blob));
    }
  });

  onCleanup(() => {
    const url = src();
    if (url) URL.revokeObjectURL(url);
  });

  return (
    <div class="photo-thumbnail">
      <Show when={src()} fallback={<div class="photo-loading">Loading...</div>}>
        <img src={src()!} alt="Recipe photo" />
      </Show>
      <button type="button" class="photo-remove" onClick={props.onRemove}>
        &times;
      </button>
    </div>
  );
}
