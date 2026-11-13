#[path = "../../src/tool_command.rs"]
mod tool_command;

use std::{ffi::OsString, path::Path};

#[test]
fn yt_dlp_uses_the_supplied_temporary_directory() {
    let directory = Path::new("a directory").join("temporary files");
    let command = tool_command::yt_dlp(&directory);
    let mut expected = OsString::from("temp:");
    expected.push(directory.as_os_str());

    assert_eq!(
        command.get_args().collect::<Vec<_>>(),
        vec![OsString::from("--paths"), expected]
    );
}

#[cfg(windows)]
#[test]
fn media_tools_run_without_a_console() {
    let output = tool_command::media_tool("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Add-Type -Name ConsoleCheck -Namespace Native -MemberDefinition '[System.Runtime.InteropServices.DllImport(\"kernel32.dll\")] public static extern System.IntPtr GetConsoleWindow();'; if ([Native.ConsoleCheck]::GetConsoleWindow() -ne [IntPtr]::Zero) { exit 1 }",
        ])
        .output()
        .expect("Could not run console-window test");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
