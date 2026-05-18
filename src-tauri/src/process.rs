use std::process::{Command, Output};

use crate::error::{stringify_io, AppResult};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[cfg(windows)]
fn hide_console(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_console(_command: &mut Command) {}

pub fn hidden_command(program: &str) -> Command {
    let mut command = Command::new(program);
    hide_console(&mut command);
    command
}

#[allow(dead_code)]
pub fn hidden_output(program: &str, args: &[&str]) -> AppResult<Output> {
    hidden_command(program)
        .args(args)
        .output()
        .map_err(stringify_io)
}
