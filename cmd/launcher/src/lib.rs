use std::path::PathBuf;

use browser_engine::{
    CamoufoxAdapter, EngineAdapter, EngineUpdateArtifact, EngineUpdatePolicy, EngineUpdateService,
    LaunchRequest, UpdateMode, WayfernAdapter,
};
use browser_profile::{CreateProfileInput, Engine, ProfileManager};
use uuid::Uuid;

pub fn run_with_args(args: &[String]) -> Result<String, String> {
    if args.is_empty() {
        return Err(help());
    }
    match args[0].as_str() {
        "init-profile" => cmd_init_profile(&args[1..]),
        "list-profiles" => cmd_list_profiles(&args[1..]),
        "ack-wayfern-tos" => cmd_ack_wayfern_tos(&args[1..]),
        "build-launch-plan" => cmd_build_launch_plan(&args[1..]),
        "update-apply" => cmd_update_apply(&args[1..]),
        _ => Err(help()),
    }
}

fn cmd_init_profile(args: &[String]) -> Result<String, String> {
    let root = parse_flag(args, "--root")?;
    let name = parse_flag(args, "--name")?;
    let engine = parse_flag(args, "--engine")?;
    let manager = ProfileManager::new(root).map_err(|e| e.to_string())?;
    let created = manager
        .create_profile(CreateProfileInput {
            name,
            description: None,
            tags: vec!["cli".to_string()],
            engine: match engine.as_str() {
                "wayfern" => Engine::Wayfern,
                "camoufox" => Engine::Camoufox,
                _ => return Err("engine must be wayfern|camoufox".to_string()),
            },
            default_start_page: Some("https://duckduckgo.com".to_string()),
            default_search_provider: None,
            ephemeral_mode: false,
            password_lock_enabled: false,
            panic_frame_enabled: false,
            panic_frame_color: None,
            panic_protected_sites: vec![],
            ephemeral_retain_paths: vec![],
        })
        .map_err(|e| e.to_string())?;
    Ok(created.id.to_string())
}

fn cmd_list_profiles(args: &[String]) -> Result<String, String> {
    let root = parse_flag(args, "--root")?;
    let manager = ProfileManager::new(root).map_err(|e| e.to_string())?;
    let mut out = String::new();
    for item in manager.list_profiles().map_err(|e| e.to_string())? {
        out.push_str(&format!("{} {} {:?}\n", item.id, item.name, item.engine));
    }
    Ok(out)
}

fn cmd_ack_wayfern_tos(args: &[String]) -> Result<String, String> {
    let root = parse_flag(args, "--root")?;
    let profile_id = parse_flag(args, "--profile-id")?
        .parse::<Uuid>()
        .map_err(|e| e.to_string())?;
    let adapter = WayfernAdapter {
        install_root: PathBuf::from(".launcher").join("engines"),
        cache_dir: PathBuf::from(".launcher").join("cache"),
        tos_version: "2026-04".to_string(),
    };
    adapter
        .acknowledge_tos(
            &PathBuf::from(root).join(profile_id.to_string()),
            profile_id,
        )
        .map_err(|e| e.to_string())?;
    Ok("ok".to_string())
}

fn cmd_build_launch_plan(args: &[String]) -> Result<String, String> {
    let root = parse_flag(args, "--root")?;
    let profile_id = parse_flag(args, "--profile-id")?
        .parse::<Uuid>()
        .map_err(|e| e.to_string())?;
    let binary = parse_flag(args, "--binary")?;
    let manager = ProfileManager::new(&root).map_err(|e| e.to_string())?;
    let profile = manager.get_profile(profile_id).map_err(|e| e.to_string())?;
    let request = LaunchRequest {
        profile_id,
        profile_root: PathBuf::from(&root).join(profile_id.to_string()),
        binary_path: PathBuf::from(binary),
        args: vec!["--profile".to_string(), profile_id.to_string()],
        env: vec![],
    };
    match profile.engine {
        Engine::Wayfern => {
            let adapter = WayfernAdapter {
                install_root: PathBuf::from(".launcher").join("engines"),
                cache_dir: PathBuf::from(".launcher").join("cache"),
                tos_version: "2026-04".to_string(),
            };
            let plan = adapter
                .build_launch_plan(request)
                .map_err(|e| e.to_string())?;
            Ok(format!("{:?}", plan.engine))
        }
        Engine::Camoufox => {
            let adapter = CamoufoxAdapter {
                install_root: PathBuf::from(".launcher").join("engines"),
                cache_dir: PathBuf::from(".launcher").join("cache"),
            };
            let plan = adapter
                .build_launch_plan(request)
                .map_err(|e| e.to_string())?;
            Ok(format!("{:?}", plan.engine))
        }
    }
}

fn cmd_update_apply(args: &[String]) -> Result<String, String> {
    let expected_signature = parse_flag(args, "--signature")?;
    let version = parse_flag(args, "--version")?;
    let policy = EngineUpdatePolicy {
        mode: UpdateMode::Manual,
        allow_user_enable: true,
    };
    let artifact = EngineUpdateArtifact {
        version,
        signature: expected_signature.clone(),
    };
    EngineUpdateService
        .verify_and_apply(&policy, &artifact, &expected_signature)
        .map_err(|e| e.to_string())
}

fn parse_flag(args: &[String], flag: &str) -> Result<String, String> {
    let mut i = 0usize;
    while i < args.len() {
        if args[i] == flag {
            if i + 1 >= args.len() {
                return Err(format!("missing value for {flag}"));
            }
            return Ok(args[i + 1].clone());
        }
        i += 1;
    }
    Err(format!("missing {flag}"))
}

pub fn help() -> String {
    [
        "usage:",
        "  launcher init-profile --root <dir> --name <name> --engine wayfern|camoufox",
        "  launcher list-profiles --root <dir>",
        "  launcher ack-wayfern-tos --root <dir> --profile-id <uuid>",
        "  launcher build-launch-plan --root <dir> --profile-id <uuid> --binary <path>",
        "  launcher update-apply --version <semver> --signature <sig>",
    ]
    .join("\n")
}
