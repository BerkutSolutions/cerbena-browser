export function createDebugLogger(scope) {
  const enabled = !!window.__BROWSER_DEV__;

  function format(level, message, extra) {
    const ts = new Date().toISOString();
    return [`[${ts}]`, `[${scope}]`, `[${level}]`, message, extra ?? ""].join(" ").trim();
  }

  return {
    debug(message, extra) {
      if (!enabled) return;
      console.debug(format("debug", message, extra));
    },
    info(message, extra) {
      if (!enabled) return;
      console.info(format("info", message, extra));
    },
    warn(message, extra) {
      if (!enabled) return;
      console.warn(format("warn", message, extra));
    },
    error(message, extra) {
      if (!enabled) return;
      console.error(format("error", message, extra));
    }
  };
}
