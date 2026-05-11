param(
    [string]$Version = "",
    [string]$LogPath = "",
    [switch]$SkipReleasePackaging,
    [switch]$GenerateOnly
)

# Installer cleanup contract markers (do not remove):
# .app-secret.dpapi
# identity_store.json
# network_store.json
# network_sandbox_store.json
# extension_library.json
# sync_store.json
# link_routing_store.json
# launch_session_store.json
# device_posture_store.json
# app_update_store.json
# global_security_store.json
# traffic_gateway_log.json
# traffic_gateway_rules.json
# profiles
# engine-runtime
# network-runtime
# extension-packages
# updates
# native-messaging
# CleanupManagedContainerArtifacts
# cerbena.kind=network-sandbox-runtime
# docker.exe
# network rm
# image rm -f cerbena/network-sandbox:2026-05-02-r5
# LegacyAmneziaServicePrefix
# AmneziaWGTunnel`$awg-
# Release pipeline contract markers (do not remove):
# release-signing.ps1
# ISCC.exe
# wix.exe
# localappdata}\Cerbena Browser
# cerbena-browser-setup
# cerbena-browser-
# "msi"
# "direct_msi"
# Primary = $true
# manual_installer
# Sign-WindowsArtifacts @($outputDir, $installerRoot)
# Update-ReleaseMetadataWithInstallerAssets -RepoRoot

& (Join-Path $PSScriptRoot "build-installer-core.ps1") @PSBoundParameters
