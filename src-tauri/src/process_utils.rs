//! 进程工具模块：创建跨平台无窗口子进程。
//!
//! 在 Windows 上设置 CREATE_NO_WINDOW 标志隐藏控制台窗口，
//! 其他平台直接使用标准 Command。

use std::process::Command;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// 创建不显示控制台窗口的子进程 Command。
///
/// Windows 上设置 CREATE_NO_WINDOW，其他平台等同于 `Command::new`。
pub fn command_no_window(program: &str) -> Command {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let mut command = Command::new(program);
        command.creation_flags(CREATE_NO_WINDOW);
        command
    }

    #[cfg(not(target_os = "windows"))]
    {
        Command::new(program)
    }
}
