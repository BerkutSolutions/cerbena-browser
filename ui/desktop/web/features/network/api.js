import { callCommand } from "../../core/commands.js";

export async function getNetworkState(profileId) {
  return callCommand("get_network_state", { profileId });
}

export async function saveVpnProxyPolicy(profileId, payload, selectedTemplateId = null) {
  return callCommand("save_vpn_proxy_policy", {
    request: { profileId, payload, selectedTemplateId }
  });
}

export async function testVpnProxyPolicy(payload) {
  return callCommand("test_vpn_proxy_policy", { payload });
}

export async function saveConnectionTemplate(request) {
  return callCommand("save_connection_template", { request });
}

export async function deleteConnectionTemplate(templateId) {
  return callCommand("delete_connection_template", { request: { templateId } });
}

export async function pingConnectionTemplate(templateId) {
  return callCommand("ping_connection_template", { request: { templateId } });
}

export async function testConnectionTemplateRequest(request) {
  return callCommand("test_connection_template_request", { request });
}

export async function saveGlobalRouteSettings(request) {
  return callCommand("save_global_route_settings", { request });
}

export async function saveDnsPolicy(profileId, payload) {
  return callCommand("save_dns_policy", { request: { profileId, payload } });
}

export async function getServiceCatalog() {
  return callCommand("get_service_catalog");
}

export async function setServiceBlockAll(category, blockAll) {
  return callCommand("set_service_block_all", { category, blockAll });
}

export async function setServiceAllowed(category, service, allowed) {
  return callCommand("set_service_allowed", { category, service, allowed });
}
