use std::{ffi::OsString, path::Path, process::Command};

pub(crate) fn media_tool(program: &str) -> Command {
    let mut command = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

pub(crate) fn yt_dlp(temp_directory: &Path) -> Command {
    let mut command = media_tool("yt-dlp");
    let mut temp_path = OsString::from("temp:");
    temp_path.push(temp_directory.as_os_str());
    command.arg("--paths").arg(temp_path);
    command
}
