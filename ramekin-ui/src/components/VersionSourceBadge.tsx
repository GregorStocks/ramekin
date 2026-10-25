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
      case "normalize":
        return "Normalized";
      case "ai_photo":
        return "AI Photo";
      case "reparse":
        return "Re-parsed";
      case "migration":
        return "Migrated";
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
      case "normalize":
      case "ai_photo":
        return "version-source-badge version-source-ai";
      default:
        return "version-source-badge";
    }
  };

  return <span class={getClassName()}>{getLabel()}</span>;
}
