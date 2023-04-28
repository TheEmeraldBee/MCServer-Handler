use std::process::{Command, Stdio};
use std::{env, thread};
use std::time::Duration;
use crate::command_watcher::CommandWatcher;
use crate::io_handler::ServerIOHandler;

pub mod command_watcher;
pub mod io_handler;

fn main() {

    // Init the server here.

    'server: loop {
        // Create and start our server.sh file.
        let mut command = Command::new("bash")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .args(["./server/run.sh"])
            .spawn()
            .unwrap();

        // First build our STDIO Handler that will handle input and console output.
        let mut stdio_handler = ServerIOHandler::new(command.stdout.take().unwrap());

        // Finally Build Our Watcher for the command that will watch the system.
        let mut command_watcher = CommandWatcher::new(command);

        // Give the server a second to start up.
        thread::sleep(Duration::from_millis(500));

        // This loop will run while the command for the server is still running.
        'command: loop {
            // Update the STDIO Handler
            // This will print out the input, and return any input it gets from stdin.
            let inputs = stdio_handler.handle_input();

            // This will print the console of the command to our stdout.
            stdio_handler.handle_output();

            // TODO: Implement Custom Commands
            for input in &inputs {
                match command_watcher.send_string(input.clone()) {
                    Ok(_) => {},
                    Err(err) => { println!("Sending input to command failed with result {err:?}") }
                }
            }

            // Check if command is complete if it is done, then our server is now idle.
            if let Some(code) = command_watcher.check_complete() {
                println!("Command exited with code {code}");
                break 'command;
            }
        }

        // Lets get some whitespace.
        println!("\n\n\n\n\n\n\n\n");

        // Set up for the idle server to run.

        'idle: loop {

        }
    }
}
