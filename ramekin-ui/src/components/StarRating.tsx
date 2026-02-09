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

  const handleKeyDown = (index: number, e: KeyboardEvent) => {
    if (!isEditable() || !props.onRate) return;
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      props.onRate(index + 1);
    }
  };

  return (
    <div
      class={`star-rating ${isEditable() ? "editable" : ""}`}
      role="group"
      aria-label={`Rating: ${props.rating ?? 0} out of 5 stars`}
      onMouseLeave={() => setHoverRating(null)}
    >
      <For each={[0, 1, 2, 3, 4]}>
        {(index) => (
          <span
            class={getStarClass(index)}
            role={isEditable() ? "button" : undefined}
            tabIndex={isEditable() ? 0 : undefined}
            aria-label={`${index + 1} star${index > 0 ? "s" : ""}`}
            onClick={() => handleClick(index)}
            onMouseEnter={() => isEditable() && setHoverRating(index + 1)}
            onKeyDown={(e) => handleKeyDown(index, e)}
          >
            ★
          </span>
        )}
      </For>
    </div>
  );
}
