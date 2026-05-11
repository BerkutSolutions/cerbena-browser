export function shellState(model) {
  return model.featureState;
}

export function profilesState(model) {
  return shellState(model).profiles;
}

export function homeState(model) {
  return shellState(model).home;
}

export function settingsState(model) {
  return shellState(model).settings;
}

export function networkState(model) {
  return shellState(model).network;
}

export function logsState(model) {
  return shellState(model).logs;
}

export function trafficState(model) {
  return shellState(model).traffic;
}

export function panicState(model) {
  return shellState(model).panic;
}
