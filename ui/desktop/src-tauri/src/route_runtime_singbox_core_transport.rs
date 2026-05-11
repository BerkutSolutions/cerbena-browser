use super::*;

#[path = "route_runtime_singbox_transport_proxy.rs"]
pub(crate) mod transport_proxy;
#[path = "route_runtime_singbox_transport_tor.rs"]
pub(crate) mod transport_tor;
#[path = "route_runtime_singbox_transport_v2ray.rs"]
pub(crate) mod transport_v2ray;
#[path = "route_runtime_singbox_transport_vpn.rs"]
pub(crate) mod transport_vpn;

pub(crate) enum SingBoxRuntimeEntry {
    Outbound(Value),
    Endpoint(Value),
}

pub(crate) fn node_to_sing_box_runtime_entry_impl(
    app_handle: &AppHandle,
    node: &NormalizedNode,
    tag: &str,
    detour: Option<String>,
    target: RuntimeExecutionTarget,
) -> Result<SingBoxRuntimeEntry, String> {
    let mut entry = match node.connection_type.as_str() {
        "proxy" => SingBoxRuntimeEntry::Outbound(transport_proxy::proxy_outbound_impl(node, tag)?),
        "v2ray" => {
            SingBoxRuntimeEntry::Outbound(transport_v2ray::v2ray_outbound_impl(node, tag)?)
        }
        "vpn" => transport_vpn::vpn_runtime_entry_impl(node, tag, target)?,
        "tor" => SingBoxRuntimeEntry::Outbound(transport_tor::tor_outbound_impl(
            app_handle, node, tag, target,
        )?),
        _ => return Err("unsupported node type for runtime".to_string()),
    };
    if let Some(detour_tag) = detour {
        let value = match &mut entry {
            SingBoxRuntimeEntry::Outbound(value) => value,
            SingBoxRuntimeEntry::Endpoint(value) => value,
        };
        if let Some(map) = value.as_object_mut() {
            map.insert("detour".to_string(), json!(detour_tag));
        }
    }
    Ok(entry)
}
