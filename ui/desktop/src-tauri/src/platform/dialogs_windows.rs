use std::process::Command;

fn run_powershell_dialog(script: &str, error_context: &str) -> Result<String, String> {
    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-Command", script])
        .output()
        .map_err(|e| format!("{error_context}: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            error_context.to_string()
        } else {
            format!("{error_context}: {stderr}")
        });
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        return Err(format!("{error_context}: selection was cancelled"));
    }
    Ok(stdout)
}

pub fn pick_folder() -> Result<String, String> {
    let script = r#"
Add-Type -AssemblyName System.Windows.Forms
$dialog = New-Object System.Windows.Forms.FolderBrowserDialog
$dialog.ShowNewFolderButton = $true
if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
  $dialog.SelectedPath | ConvertTo-Json -Compress
}
"#;
    let stdout = run_powershell_dialog(script, "folder picker failed")?;
    serde_json::from_str::<String>(&stdout).map_err(|e| format!("folder picker parse failed: {e}"))
}

pub fn pick_file(title: &str, filter: &str) -> Result<String, String> {
    let script = format!(
        r#"
Add-Type -AssemblyName System.Windows.Forms
$dialog = New-Object System.Windows.Forms.OpenFileDialog
$dialog.Filter = '{filter}'
$dialog.Title = '{title}'
$dialog.Multiselect = $false
$dialog.CheckFileExists = $true
$dialog.CheckPathExists = $true
if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {{
  $dialog.FileName | ConvertTo-Json -Compress
}}
"#
    );
    let stdout = run_powershell_dialog(&script, "file picker failed")?;
    serde_json::from_str::<String>(&stdout).map_err(|e| format!("file picker parse failed: {e}"))
}

pub fn pick_certificate_files() -> Result<Vec<String>, String> {
    let script = r#"
Add-Type -AssemblyName System.Windows.Forms
$dialog = New-Object System.Windows.Forms.OpenFileDialog
$dialog.Filter = 'Certificates (*.pem;*.crt;*.cer)|*.pem;*.crt;*.cer'
$dialog.Multiselect = $true
$dialog.CheckFileExists = $true
$dialog.CheckPathExists = $true
if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
  $dialog.FileNames | ConvertTo-Json -Compress
}
"#;
    let stdout = run_powershell_dialog(script, "certificate picker failed")?;
    serde_json::from_str::<Vec<String>>(&stdout)
        .or_else(|_| serde_json::from_str::<String>(&stdout).map(|item| vec![item]))
        .map_err(|e| format!("certificate picker parse failed: {e}"))
}
