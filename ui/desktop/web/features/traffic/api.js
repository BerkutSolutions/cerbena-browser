import { callCommand } from "../../core/commands.js";

export async function listTrafficEvents() {
  return callCommand("list_traffic_events");
}

export async function setTrafficRule(profileId, domain, blocked) {
  return callCommand("set_traffic_rule", { request: { profileId, domain, blocked } });
}
