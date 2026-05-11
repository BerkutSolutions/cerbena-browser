use super::*;
use crate::state::{NetworkGlobalRouteSettings, NetworkStore};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use browser_network_policy::VpnProxyTabPayload;
use flate2::{write::ZlibEncoder, Compression};
use std::collections::BTreeMap;
use std::io::Write;

    fn build_amnezia_key(payload: &str) -> String {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(payload.as_bytes())
            .expect("write amnezia payload");
        let compressed = encoder.finish().expect("finish amnezia compression");
        let mut framed = Vec::with_capacity(compressed.len() + 4);
        let len = payload.len() as u32;
        framed.extend_from_slice(&len.to_be_bytes());
        framed.extend_from_slice(&compressed);
        format!("vpn://{}", URL_SAFE_NO_PAD.encode(framed))
    }

    #[test]
    fn parse_amnezia_runtime_config_extracts_wireguard_settings() {
        let payload = r#"{
          "hostName":"91.186.212.196",
          "containers":[
            {
              "awg":{
                "transport_proto":"udp",
                "last_config":"{\"client_priv_key\":\"PRIVATE\",\"server_pub_key\":\"PUBLIC\",\"psk_key\":\"PSK\",\"client_ip\":\"10.8.1.6\",\"allowed_ips\":[\"0.0.0.0/0\",\"::/0\"],\"persistent_keep_alive\":\"25\",\"mtu\":\"1376\",\"port\":\"44017\",\"hostName\":\"91.186.212.196\"}"
              }
            }
          ]
        }"#;
        let key = build_amnezia_key(payload);
        let cfg = parse_amnezia_runtime_config(&key).expect("parse amnezia runtime config");
        assert_eq!(cfg.host, "91.186.212.196");
        assert_eq!(cfg.port, 44017);
        assert_eq!(cfg.client_private_key, "PRIVATE");
        assert_eq!(cfg.server_public_key, "PUBLIC");
        assert_eq!(cfg.pre_shared_key.as_deref(), Some("PSK"));
        assert_eq!(cfg.addresses, vec!["10.8.1.6/32".to_string()]);
        assert_eq!(
            cfg.allowed_ips,
            vec!["0.0.0.0/0".to_string(), "::/0".to_string()]
        );
        assert_eq!(cfg.mtu, Some(1376));
        assert_eq!(cfg.transport.as_deref(), Some("udp"));
    }

    #[test]
    fn parse_amnezia_runtime_config_supports_awg_conf() {
        let conf = r#"
[Interface]
Address = 10.8.1.84/32
DNS = 1.2.1.1, 1.0.0.1
PrivateKey = PRIVATE
Jc = 4
Jmin = 10
Jmax = 50

[Peer]
PublicKey = PUBLIC
PresharedKey = PSK
AllowedIPs = 0.0.0.0/0, ::/0
Endpoint = 5.129.225.48:32542
PersistentKeepalive = 25
"#;
        let cfg = parse_amnezia_runtime_config(conf).expect("parse amnezia conf runtime config");
        assert_eq!(cfg.host, "5.129.225.48");
        assert_eq!(cfg.port, 32542);
        assert_eq!(cfg.client_private_key, "PRIVATE");
        assert_eq!(cfg.server_public_key, "PUBLIC");
        assert_eq!(cfg.pre_shared_key.as_deref(), Some("PSK"));
        assert_eq!(cfg.addresses, vec!["10.8.1.84/32".to_string()]);
        assert_eq!(
            cfg.allowed_ips,
            vec!["0.0.0.0/0".to_string(), "::/0".to_string()]
        );
    }

    #[test]
    fn build_amnezia_native_config_text_from_key_replaces_dns_placeholders() {
        let last_cfg = serde_json::json!({
            "config": "[Interface]\nAddress = 10.8.1.84/32\nDNS = $PRIMARY_DNS, $SECONDARY_DNS\nPrivateKey = PRIVATE\nJc = 4\nJmin = 10\nJmax = 50\nS1 = 88\nS2 = 143\nH1 = 755270012\nH2 = 876050617\nH3 = 220715218\nH4 = 1577770230\nI1 = \nI2 = \nI3 = \nI4 = \nI5 = \n\n[Peer]\nPublicKey = PUBLIC\nPresharedKey = PSK\nAllowedIPs = 0.0.0.0/0, ::/0\nEndpoint = 5.129.225.48:32542\nPersistentKeepalive = 25\n"
        })
        .to_string();
        let payload = serde_json::json!({
            "dns1": "1.2.1.1",
            "dns2": "1.0.0.1",
            "containers": [
                {
                    "awg": {
                        "last_config": last_cfg
                    }
                }
            ]
        })
        .to_string();
        let key = build_amnezia_key(&payload);
        let conf = build_amnezia_native_config_text(&key).expect("materialize amnezia config");
        assert!(conf.contains("DNS = 1.2.1.1, 1.0.0.1"));
        assert!(conf.contains("Jc = 4"));
        assert!(conf.contains("Endpoint = 5.129.225.48:32542"));
    }

    #[test]
    fn sanitize_amnezia_conf_text_skips_empty_quoted_native_fields() {
        let raw = "[Interface]\nPrivateKey = PRIVATE\nI1 = \nI2=''\nI3 = \"\"\nI4 =   \nI5 = 12345\nJc = 4\n\n[Peer]\nPublicKey = PUBLIC\n";
        let sanitized = sanitize_amnezia_conf_text(raw);
        assert!(sanitized.contains("PrivateKey = PRIVATE"));
        assert!(sanitized.contains("Jc = 4"));
        assert!(sanitized.contains("I5 = 12345"));
        assert!(!sanitized.contains("I1 ="));
        assert!(!sanitized.contains("I2 ="));
        assert!(!sanitized.contains("I3 ="));
        assert!(!sanitized.contains("I4 ="));
    }

    #[test]
    fn build_amnezia_native_config_text_from_key_skips_empty_quoted_native_fields() {
        let payload = serde_json::json!({
            "dns1": "1.2.1.1",
            "containers": [
                {
                    "awg": {
                        "client_priv_key": "PRIVATE",
                        "server_pub_key": "PUBLIC",
                        "client_ip": "10.8.1.84/32",
                        "allowed_ips": ["0.0.0.0/0", "::/0"],
                        "port": "32542",
                        "hostName": "5.129.225.48",
                        "Jc": "4",
                        "I1": "",
                        "I2": "''",
                        "I3": "\"\"",
                        "I4": "  ",
                        "I5": "12345",
                        "persistentKeepalive": "25"
                    }
                }
            ]
        })
        .to_string();
        let key = build_amnezia_key(&payload);
        let conf = build_amnezia_native_config_text(&key).expect("materialize amnezia config");
        assert!(conf.contains("Jc = 4"));
        assert!(conf.contains("I5 = 12345"));
        assert!(!conf.contains("I1 ="));
        assert!(!conf.contains("I2 ="));
        assert!(!conf.contains("I3 ="));
        assert!(!conf.contains("I4 ="));
    }

    #[test]
    fn amnezia_tunnel_name_is_stable_and_within_limit() {
        let profile_id = Uuid::parse_str("7aafb8b9-17f0-4c2b-8dac-b92e77629d44").expect("uuid");
        let name = amnezia_tunnel_name(profile_id);
        assert!(name.starts_with("awg-"));
        assert!(name.len() <= 32);
    }

    #[test]
    fn single_amnezia_profile_uses_sing_box_runtime_tools() {
        let nodes = vec![NormalizedNode {
            connection_type: "vpn".to_string(),
            protocol: "amnezia".to_string(),
            host: Some("demo.example".to_string()),
            port: Some(443),
            username: None,
            password: None,
            bridges: None,
            settings: BTreeMap::from([(
                "amneziaKey".to_string(),
                build_amnezia_key(
                    r#"{"containers":[{"awg":{"last_config":"{\"client_priv_key\":\"PRIVATE\",\"server_pub_key\":\"PUBLIC\",\"client_ip\":\"10.0.0.2\",\"allowed_ips\":[\"0.0.0.0/0\"],\"port\":\"443\",\"hostName\":\"demo.example\"}"}}]}"#,
                ),
            )]),
        }];
        let tools = required_runtime_tools(&nodes, false, false, false, false);
        assert!(tools.contains(&NetworkTool::SingBox));
        assert!(!tools.contains(&NetworkTool::AmneziaWg));
    }

    #[test]
    fn resolve_effective_route_selection_prioritizes_direct_profile_over_global_vpn() {
        let profile_key = "profile-direct-priority".to_string();
        let mut store = NetworkStore::default();
        store.vpn_proxy.insert(
            profile_key.clone(),
            VpnProxyTabPayload {
                route_mode: "direct".to_string(),
                proxy: None,
                vpn: None,
                kill_switch_enabled: false,
            },
        );
        store
            .profile_template_selection
            .insert(profile_key.clone(), "profile-template".to_string());
        store.global_route_settings = NetworkGlobalRouteSettings {
            global_vpn_enabled: true,
            block_without_vpn: true,
            default_template_id: Some("global-template".to_string()),
        };

        let (mode, template) = resolve_effective_route_selection(&store, &profile_key);
        assert_eq!(mode, "direct");
        assert_eq!(template, None);
    }

    #[test]
    fn resolve_effective_route_selection_uses_global_defaults_for_non_direct_profiles() {
        let profile_key = "profile-global-default".to_string();
        let mut store = NetworkStore::default();
        store.vpn_proxy.insert(
            profile_key.clone(),
            VpnProxyTabPayload {
                route_mode: "vpn".to_string(),
                proxy: None,
                vpn: None,
                kill_switch_enabled: true,
            },
        );
        store
            .profile_template_selection
            .insert(profile_key.clone(), "profile-template".to_string());
        store.global_route_settings = NetworkGlobalRouteSettings {
            global_vpn_enabled: true,
            block_without_vpn: true,
            default_template_id: Some("global-template".to_string()),
        };

        let (mode, template) = resolve_effective_route_selection(&store, &profile_key);
        assert_eq!(mode, "vpn");
        assert_eq!(template.as_deref(), Some("global-template"));
    }

    #[test]
    fn stale_runtime_cleanup_targets_known_launcher_artifacts_only() {
        assert!(should_remove_runtime_artifact("sing-box-route.json"));
        assert!(should_remove_runtime_artifact("openvpn-route.ovpn"));
        assert!(should_remove_runtime_artifact("container-openvpn.ovpn"));
        assert!(should_remove_runtime_artifact("openvpn-auth-demo.txt"));
        assert!(should_remove_runtime_artifact("awg-test.conf"));
        assert!(!should_remove_runtime_artifact("notes.txt"));
    }

    #[test]
    fn openvpn_raw_config_requires_auth_file_when_profile_requests_auth_user_pass() {
        let node = NormalizedNode {
            connection_type: "vpn".to_string(),
            protocol: "openvpn".to_string(),
            host: Some("demo.example".to_string()),
            port: Some(1194),
            username: None,
            password: None,
            bridges: None,
            settings: BTreeMap::from([(
                "ovpnRaw".to_string(),
                "client\nauth-user-pass\nremote demo.example 1194 udp4\n".to_string(),
            )]),
        };
        let log_path = PathBuf::from("/tmp/openvpn.log");

        let error =
            build_openvpn_config_text(&node, None, &log_path).expect_err("auth-user-pass error");
        assert_eq!(
            error,
            "openvpn profile requests auth-user-pass; set username/password fields"
        );

        let config = build_openvpn_config_text(
            &node,
            Some(&PathBuf::from("/work/openvpn-auth.txt")),
            &PathBuf::from("/work/route.log"),
        )
        .expect("openvpn config with auth path");
        assert!(config.contains("auth-user-pass \"/work/openvpn-auth.txt\""));
        assert!(!config.contains("\nnobind\n"));
        assert!(config.contains("\nremote demo.example 1194 udp4\n"));
    }

    #[test]
    fn container_tor_transport_binary_covers_supported_transports() {
        assert_eq!(
            container_tor_transport_binary("obfs4").as_deref(),
            Some("/usr/bin/obfs4proxy")
        );
        assert_eq!(
            container_tor_transport_binary("snowflake").as_deref(),
            Some("/usr/bin/snowflake-client")
        );
        assert_eq!(
            container_tor_transport_binary("meek").as_deref(),
            Some("/usr/bin/obfs4proxy")
        );
        assert!(container_tor_transport_binary("unknown").is_none());
    }
