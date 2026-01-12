interface VersionSourceBadgeProps {
  source: string;
}

export default function VersionSourceBadge(props: VersionSourceBadgeProps) {
  const getLabel = () => {
    switch (props.source) {
      case "user":
        return "User Edit";
      case "scrape":
        return "Imported";
      case "enrich":
        return "AI Enriched";
      default:
        return props.source;
    }
  };

  const getClassName = () => {
    switch (props.source) {
      case "user":
        return "version-source-badge version-source-user";
      case "scrape":
        return "version-source-badge version-source-scrape";
      case "enrich":
        return "version-source-badge version-source-enrich";
      default:
        return "version-source-badge";
    }
  };

  return <span class={getClassName()}>{getLabel()}</span>;
}
