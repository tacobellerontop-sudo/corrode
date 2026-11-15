export type LogLevel = "debug" | "info" | "warn" | "error";

export interface Logger {
  debug(...args: unknown[]): void;
  info(...args: unknown[]): void;
  warn(...args: unknown[]): void;
  error(...args: unknown[]): void;
  child(name: string): Logger;
}

const LEVEL_STYLE: Record<LogLevel, string> = {
  debug: "color:#949ba4",
  info: "color:#e0884b",
  warn: "color:#f0b232",
  error: "color:#f23f43",
};

const BADGE = "background:#b7410e;color:#fff;border-radius:3px;padding:0 4px;font-weight:700";

/** Console logger with a "Corrode" badge and a scope name, e.g. `[Corrode] [Webpack] ...`. */
export function createLogger(scope?: string): Logger {
  const log =
    (level: LogLevel) =>
    (...args: unknown[]) => {
      const method = level === "debug" ? console.debug : console[level];
      if (scope) method.call(console, `%cCorrode%c %c[${scope}]`, BADGE, "", LEVEL_STYLE[level], ...args);
      else method.call(console, "%cCorrode", BADGE, ...args);
    };
  return {
    debug: log("debug"),
    info: log("info"),
    warn: log("warn"),
    error: log("error"),
    child: (name) => createLogger(scope ? `${scope} > ${name}` : name),
  };
}

export const logger = createLogger();
