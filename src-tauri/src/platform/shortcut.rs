//! Creates a Start Menu shortcut with AppUserModelID on Windows.
//!
//! Windows SMTC reads the app name from the Start Menu shortcut that has a matching
//! AppUserModelID. Without this shortcut, SMTC shows "Unknown app".
//! 
//! We use PowerShell to create the shortcut since the Windows Shell COM APIs
//! are complex to use correctly from Rust.

#[cfg(target_os = "windows")]
pub fn ensure_start_menu_shortcut(app_user_model_id: &str, display_name: &str) {
    use std::os::windows::process::CommandExt;
    use tracing::{info, warn};

    // Get the current executable path
    let exe_path = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            warn!("Failed to get current exe path: {:?}", e);
            return;
        }
    };

    // Get Start Menu Programs folder: %APPDATA%\Microsoft\Windows\Start Menu\Programs
    let appdata = match std::env::var("APPDATA") {
        Ok(a) => a,
        Err(e) => {
            warn!("Failed to get APPDATA: {:?}", e);
            return;
        }
    };

    let shortcut_dir = std::path::PathBuf::from(&appdata)
        .join("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs");

    let shortcut_path = shortcut_dir.join(format!("{}.lnk", display_name));

    // We used to skip if it existed, but we need to ensure the AUMID is updated
    // in case it was created with an old incorrect ID.
    if shortcut_path.exists() {
        info!("Updating existing Start Menu shortcut: {:?}", shortcut_path);
    }

    info!(
        "Creating Start Menu shortcut at {:?} for {:?}",
        shortcut_path, exe_path
    );

    // Use PowerShell to create shortcut with AppUserModelID
    // This is more reliable than using the Windows COM API directly from Rust
    let lnk_filename = format!("{}.lnk", display_name);
    let ps_script = format!(
        r#"
$shell = New-Object -ComObject WScript.Shell
$shortcut = $shell.CreateShortcut('{}')
$shortcut.TargetPath = '{}'
$shortcut.Description = '{}'
$shortcut.Save()

# Now set AppUserModelID property on the .lnk file
$shell2 = New-Object -ComObject Shell.Application
$dir = $shell2.NameSpace('{}')
$lnk = $dir.ParseName('{}')
# Verify shortcut was created
if (Test-Path '{}') {{
    Write-Host 'Shortcut created successfully'
}} else {{
    Write-Host 'Failed to create shortcut'
}}
"#,
        shortcut_path.to_string_lossy().replace('\'', "''"),
        exe_path.to_string_lossy().replace('\'', "''"),
        display_name,
        shortcut_dir.to_string_lossy().replace('\'', "''"),
        lnk_filename,
        shortcut_path.to_string_lossy().replace('\'', "''"),
    );

    match std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &ps_script])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .output()
    {
        Ok(output) => {
            if output.status.success() {
                info!("Start Menu shortcut created successfully");
                // Now set AppUserModelID using separate PowerShell call with .NET interop
                set_shortcut_aumid(&shortcut_path.to_string_lossy(), app_user_model_id);
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                warn!("PowerShell shortcut creation failed: {}", stderr);
            }
        }
        Err(e) => {
            warn!("Failed to run PowerShell: {:?}", e);
        }
    }
}

#[cfg(target_os = "windows")]
fn set_shortcut_aumid(shortcut_path: &str, app_user_model_id: &str) {
    use std::os::windows::process::CommandExt;
    use tracing::{info, warn};

    // Use PowerShell with Shell32 COM to set AppUserModelID property
    let ps_script = format!(
        r#"
# Load Shell32 for property setting
$type = [Type]::GetTypeFromCLSID('{{9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3}}')
try {{
    Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
using System.Runtime.InteropServices.ComTypes;

public class ShortcutHelper {{
    [DllImport("shell32.dll", SetLastError = true)]
    static extern int SHGetPropertyStoreFromParsingName(
        [MarshalAs(UnmanagedType.LPWStr)] string pszPath,
        IntPtr pbc,
        int flags,
        ref Guid iid,
        [MarshalAs(UnmanagedType.Interface)] out object ppv);

    [DllImport("ole32.dll")]
    static extern int PropVariantClear(ref PROPVARIANT pvar);

    [StructLayout(LayoutKind.Sequential)]
    public struct PROPERTYKEY {{
        public Guid fmtid;
        public uint pid;
    }}

    [StructLayout(LayoutKind.Sequential)]
    public struct PROPVARIANT {{
        public ushort vt;
        public ushort wReserved1;
        public ushort wReserved2;
        public ushort wReserved3;
        public IntPtr p;
        public int p2;
    }}

    [ComImport, Guid("886D8EEB-8CF2-4446-8D02-CDBA1DBDCF99"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    interface IPropertyStore {{
        int GetCount(out uint cProps);
        int GetAt(uint iProp, out PROPERTYKEY pkey);
        int GetValue(ref PROPERTYKEY key, out PROPVARIANT pv);
        int SetValue(ref PROPERTYKEY key, ref PROPVARIANT pv);
        int Commit();
    }}

    public static void SetAppUserModelId(string shortcutPath, string appId) {{
        Guid IID_IPropertyStore = new Guid("886D8EEB-8CF2-4446-8D02-CDBA1DBDCF99");
        object store;
        int hr = SHGetPropertyStoreFromParsingName(shortcutPath, IntPtr.Zero, 2 /* GPS_READWRITE */, ref IID_IPropertyStore, out store);
        if (hr != 0) throw new COMException("SHGetPropertyStoreFromParsingName failed", hr);

        IPropertyStore ps = (IPropertyStore)store;
        PROPERTYKEY appUserModelIDKey = new PROPERTYKEY();
        appUserModelIDKey.fmtid = new Guid("9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3");
        appUserModelIDKey.pid = 5;

        PROPVARIANT pv = new PROPVARIANT();
        pv.vt = 31; // VT_LPWSTR
        pv.p = Marshal.StringToCoTaskMemUni(appId);
        ps.SetValue(ref appUserModelIDKey, ref pv);
        ps.Commit();
        Marshal.FreeCoTaskMem(pv.p);
        Marshal.ReleaseComObject(store);
    }}
}}
"@

    [ShortcutHelper]::SetAppUserModelId('{}', '{}')
    Write-Host 'AppUserModelID set successfully'
}} catch {{
    Write-Host "Failed to set AppUserModelID: $_"
}}
"#,
        shortcut_path.replace('\'', "''"),
        app_user_model_id.replace('\'', "''"),
    );

    match std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &ps_script])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .output()
    {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            if output.status.success() {
                info!("AppUserModelID set: {}", stdout.trim());
            } else {
                warn!("Failed to set AppUserModelID: {} {}", stdout.trim(), stderr.trim());
            }
        }
        Err(e) => {
            warn!("Failed to run PowerShell for AUMID: {:?}", e);
        }
    }
}

#[cfg(not(target_os = "windows"))]
#[allow(dead_code)]
pub fn ensure_start_menu_shortcut(_app_user_model_id: &str, _display_name: &str) {
    // No-op on non-Windows platforms
}
