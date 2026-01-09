import { createSignal, For } from "solid-js";

interface StarRatingProps {
  rating: number | null;
  onRate?: (rating: number) => void;
  readonly?: boolean;
}

export default function StarRating(props: StarRatingProps) {
  const [hoverRating, setHoverRating] = createSignal<number | null>(null);

  const isEditable = () => !props.readonly && props.onRate;

  const getStarClass = (index: number) => {
    const displayRating = hoverRating() ?? props.rating ?? 0;
    const filled = index < displayRating;
    return `star ${filled ? "filled" : ""}`;
  };

  const handleClick = (index: number) => {
    if (isEditable() && props.onRate) {
      props.onRate(index + 1);
    }
  };

  return (
    <div
      class={`star-rating ${isEditable() ? "editable" : ""}`}
      onMouseLeave={() => setHoverRating(null)}
    >
      <For each={[0, 1, 2, 3, 4]}>
        {(index) => (
          <span
            class={getStarClass(index)}
            onClick={() => handleClick(index)}
            onMouseEnter={() => isEditable() && setHoverRating(index + 1)}
          >
            ★
          </span>
        )}
      </For>
    </div>
  );
}
