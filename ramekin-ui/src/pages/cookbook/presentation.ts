export type Density = "card" | "compact" | "list";

const DENSITY_KEY = "cookbookDensity";

export function loadDensity(): Density {
  const value = localStorage.getItem(DENSITY_KEY);
  return value === "compact" || value === "list" ? value : "card";
}

export function saveDensity(density: Density): void {
  localStorage.setItem(DENSITY_KEY, density);
}

export function formatRelativeDate(date: Date, now = new Date()): string {
  const diffMs = now.getTime() - date.getTime();
  const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

  if (diffDays === 0) return "Updated today";
  if (diffDays === 1) return "Updated yesterday";
  if (diffDays < 7) return `Updated ${diffDays} days ago`;
  return `Updated ${date.toLocaleDateString("en-US", {
    month: "short",
    day: "numeric",
  })}`;
}
