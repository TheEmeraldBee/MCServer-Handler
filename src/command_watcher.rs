use std::io::Write;
use std::process::{Child, ExitStatus};

pub struct CommandWatcher {
    command: Child
}

impl CommandWatcher {
    pub fn new(command: Child) -> Self {
        Self {
            command
        }
    }

    pub fn check_complete(&mut self) -> Option<ExitStatus> {
        if let Ok(code) = self.command.try_wait() {
            return code;
        }
        None
    }

    pub fn send_string(&mut self, write: String) -> std::io::Result<()> {
        if self.check_complete().is_some() {
            return Ok(());
        }

        self.command.stdin.as_mut().unwrap().write_all(write.as_bytes())?;
        self.command.stdin.as_mut().unwrap().flush().unwrap();

        return Ok(())
    }
}