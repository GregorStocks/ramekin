export type LogLevel = "log" | "warn" | "error";

export interface LogEntry {
  timestamp: string;
  level: LogLevel;
  source: string;
  message: string;
}

/** Mirrors the iOS DebugLogger's shape: timestamped source-tagged lines. */
const MAX_ENTRIES = 1000;

const buffer: LogEntry[] = [];

function record(level: LogLevel, source: string, message: string): void {
  buffer.push({
    timestamp: new Date().toISOString(),
    level,
    source,
    message,
  });
  if (buffer.length > MAX_ENTRIES) {
    buffer.splice(0, buffer.length - MAX_ENTRIES);
  }
  console[level](`[${source}] ${message}`);
}

export const logger = {
  log(source: string, message: string): void {
    record("log", source, message);
  },

  warn(source: string, message: string): void {
    record("warn", source, message);
  },

  error(source: string, message: string): void {
    record("error", source, message);
  },

  /** Logs start/completion (with elapsed ms) around an async operation. */
  async timed<T>(
    source: string,
    label: string,
    fn: () => Promise<T>,
  ): Promise<T> {
    record("log", source, `${label} started`);
    const start = performance.now();
    try {
      const result = await fn();
      const elapsed = Math.round(performance.now() - start);
      record("log", source, `${label} completed (${elapsed}ms)`);
      return result;
    } catch (err) {
      const elapsed = Math.round(performance.now() - start);
      record(
        "error",
        source,
        `${label} FAILED after ${elapsed}ms: ${String(err)}`,
      );
      throw err;
    }
  },

  /** Formats the buffer as text lines for upload. */
  dump(): string {
    return buffer
      .map((e) => `${e.timestamp} [${e.level}] [${e.source}] ${e.message}`)
      .join("\n");
  },

  entries(): readonly LogEntry[] {
    return buffer.slice();
  },

  clear(): void {
    buffer.length = 0;
  },
};
