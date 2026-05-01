export function nextCorrelationId() {
  return crypto.randomUUID();
}

export function responseEnvelope(ok, data, messageKey, correlationId) {
  return { ok, data, messageKey, correlationId };
}
