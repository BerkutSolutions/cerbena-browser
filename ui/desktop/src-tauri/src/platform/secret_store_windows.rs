use std::{fs, path::Path, process::Command};

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use rand::RngCore;

pub fn derive_app_secret_material(
    app_data_dir: &Path,
    _current_exe: &Path,
    identifier: &str,
) -> Result<String, String> {
    let path = app_data_dir.join(".app-secret.dpapi");
    let entropy = build_dpapi_entropy(app_data_dir, identifier);
    if path.exists() {
        let protected = fs::read(&path)
            .map_err(|e| format!("read protected app secret {}: {e}", path.display()))?;
        let secret = dpapi_unprotect(&protected, &entropy)?;
        return Ok(B64.encode(secret));
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("create protected app secret dir {}: {e}", parent.display()))?;
    }

    let mut secret = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut secret);
    let protected = dpapi_protect(&secret, &entropy)?;
    fs::write(&path, &protected)
        .map_err(|e| format!("write protected app secret {}: {e}", path.display()))?;
    let encoded = B64.encode(secret);
    secret.fill(0);
    Ok(encoded)
}

fn build_dpapi_entropy(app_data_dir: &Path, identifier: &str) -> Vec<u8> {
    format!("{}|{}", identifier.trim(), app_data_dir.to_string_lossy()).into_bytes()
}

fn dpapi_protect(plaintext: &[u8], entropy: &[u8]) -> Result<Vec<u8>, String> {
    run_powershell_crypto("Protect", plaintext, entropy)
}

fn dpapi_unprotect(ciphertext: &[u8], entropy: &[u8]) -> Result<Vec<u8>, String> {
    run_powershell_crypto("Unprotect", ciphertext, entropy)
}

fn run_powershell_crypto(action: &str, input: &[u8], entropy: &[u8]) -> Result<Vec<u8>, String> {
    let script = r#"
Add-Type -TypeDefinition @"
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;

public static class CerbenaDpapi {
  [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
  private struct DATA_BLOB {
    public int cbData;
    public IntPtr pbData;
  }

  [DllImport("crypt32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
  private static extern bool CryptProtectData(
    ref DATA_BLOB pDataIn,
    string szDataDescr,
    ref DATA_BLOB pOptionalEntropy,
    IntPtr pvReserved,
    IntPtr pPromptStruct,
    int dwFlags,
    out DATA_BLOB pDataOut);

  [DllImport("crypt32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
  private static extern bool CryptUnprotectData(
    ref DATA_BLOB pDataIn,
    IntPtr ppszDataDescr,
    ref DATA_BLOB pOptionalEntropy,
    IntPtr pvReserved,
    IntPtr pPromptStruct,
    int dwFlags,
    out DATA_BLOB pDataOut);

  [DllImport("kernel32.dll", SetLastError = true)]
  private static extern IntPtr LocalFree(IntPtr hMem);

  private static DATA_BLOB ToBlob(byte[] data) {
    var blob = new DATA_BLOB();
    if (data == null || data.Length == 0) {
      blob.cbData = 0;
      blob.pbData = IntPtr.Zero;
      return blob;
    }
    blob.cbData = data.Length;
    blob.pbData = Marshal.AllocHGlobal(data.Length);
    Marshal.Copy(data, 0, blob.pbData, data.Length);
    return blob;
  }

  private static byte[] CopyAndFree(DATA_BLOB blob) {
    if (blob.pbData == IntPtr.Zero || blob.cbData <= 0) {
      return Array.Empty<byte>();
    }
    try {
      var bytes = new byte[blob.cbData];
      Marshal.Copy(blob.pbData, bytes, 0, blob.cbData);
      return bytes;
    } finally {
      LocalFree(blob.pbData);
    }
  }

  private static void FreeInput(DATA_BLOB blob) {
    if (blob.pbData != IntPtr.Zero) {
      Marshal.FreeHGlobal(blob.pbData);
    }
  }

  public static byte[] Protect(byte[] data, byte[] entropy) {
    var input = ToBlob(data);
    var optionalEntropy = ToBlob(entropy);
    try {
      DATA_BLOB output;
      if (!CryptProtectData(ref input, null, ref optionalEntropy, IntPtr.Zero, IntPtr.Zero, 0, out output)) {
        throw new Win32Exception(Marshal.GetLastWin32Error());
      }
      return CopyAndFree(output);
    } finally {
      FreeInput(input);
      FreeInput(optionalEntropy);
    }
  }

  public static byte[] Unprotect(byte[] data, byte[] entropy) {
    var input = ToBlob(data);
    var optionalEntropy = ToBlob(entropy);
    try {
      DATA_BLOB output;
      if (!CryptUnprotectData(ref input, IntPtr.Zero, ref optionalEntropy, IntPtr.Zero, IntPtr.Zero, 0, out output)) {
        throw new Win32Exception(Marshal.GetLastWin32Error());
      }
      return CopyAndFree(output);
    } finally {
      FreeInput(input);
      FreeInput(optionalEntropy);
    }
  }
}
"@
$mode = '__MODE__'
$inputBytes = [Convert]::FromBase64String('__INPUT_B64__')
$entropyBytes = [Convert]::FromBase64String('__ENTROPY_B64__')
if ($mode -eq 'Protect') {
  $result = [CerbenaDpapi]::Protect($inputBytes, $entropyBytes)
} elseif ($mode -eq 'Unprotect') {
  $result = [CerbenaDpapi]::Unprotect($inputBytes, $entropyBytes)
} else {
  throw 'unsupported mode'
}
[Console]::Out.Write([Convert]::ToBase64String($result))
"#
    .replace("__MODE__", action)
    .replace("__INPUT_B64__", &B64.encode(input))
    .replace("__ENTROPY_B64__", &B64.encode(entropy));

    let mut command = Command::new("powershell");
    command.args([
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        script.as_str(),
    ]);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let output = command
        .output()
        .map_err(|e| format!("run powershell DPAPI {action}: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        return Err(format!(
            "powershell DPAPI {action} failed (code {:?}){}{}",
            output.status.code(),
            if stderr.is_empty() {
                String::new()
            } else {
                format!(" stderr: {stderr}")
            },
            if stdout.is_empty() {
                String::new()
            } else {
                format!(" stdout: {stdout}")
            }
        ));
    }
    let text = String::from_utf8(output.stdout)
        .map_err(|e| format!("decode powershell DPAPI {action} output: {e}"))?;
    B64.decode(text.trim())
        .map_err(|e| format!("decode powershell DPAPI {action} base64: {e}"))
}
