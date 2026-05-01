export function createEventBus() {
  const listeners = new Map();

  function on(event, handler) {
    const bucket = listeners.get(event) ?? [];
    bucket.push(handler);
    listeners.set(event, bucket);
    return () => {
      const next = (listeners.get(event) ?? []).filter((item) => item !== handler);
      listeners.set(event, next);
    };
  }

  function emit(event, payload) {
    for (const handler of listeners.get(event) ?? []) {
      handler(payload);
    }
  }

  return { on, emit };
}
