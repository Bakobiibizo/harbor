type LogLevel = "debug" | "info" | "warn" | "error";

const LOG_PREFIX = "[Harbor]";

const isDev = import.meta.env.DEV;

function formatMessage(level: LogLevel, module: string, message: string): string {
  const timestamp = new Date().toISOString();
  return `${LOG_PREFIX} ${timestamp} [${level.toUpperCase()}] [${module}] ${message}`;
}

function shouldLog(level: LogLevel): boolean {
  if (isDev) return true;
  return level === "warn" || level === "error";
}

export const logger = {
  debug(module: string, message: string, data?: unknown): void {
    if (!shouldLog("debug")) return;
    const formatted = formatMessage("debug", module, message);
    if (data !== undefined) {
      console.debug(formatted, data);
    } else {
      console.debug(formatted);
    }
  },

  info(module: string, message: string, data?: unknown): void {
    if (!shouldLog("info")) return;
    const formatted = formatMessage("info", module, message);
    if (data !== undefined) {
      console.info(formatted, data);
    } else {
      console.info(formatted);
    }
  },

  warn(module: string, message: string, data?: unknown): void {
    if (!shouldLog("warn")) return;
    const formatted = formatMessage("warn", module, message);
    if (data !== undefined) {
      console.warn(formatted, data);
    } else {
      console.warn(formatted);
    }
  },

  error(module: string, message: string, error?: unknown): void {
    if (!shouldLog("error")) return;
    const formatted = formatMessage("error", module, message);
    if (error !== undefined) {
      console.error(formatted, error);
    } else {
      console.error(formatted);
    }
  },
};

export function createLogger(module: string) {
  return {
    debug: (message: string, data?: unknown) => logger.debug(module, message, data),
    info: (message: string, data?: unknown) => logger.info(module, message, data),
    warn: (message: string, data?: unknown) => logger.warn(module, message, data),
    error: (message: string, error?: unknown) => logger.error(module, message, error),
  };
}
