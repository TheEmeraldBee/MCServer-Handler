use std::process::{Command, Stdio};
use std::{fs, thread};
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use serde_json::{json, Map, Value as Json};
use std::net::TcpListener;
use std::sync::mpsc;
use std::time::Duration;
use handlebars::{Handlebars, to_json};
use crate::command_watcher::CommandWatcher;
use crate::io_handler::ServerIOHandler;

pub mod command_watcher;
pub mod io_handler;

fn main() {

    let (tcp_sender, tcp_receiver) = mpsc::channel();

    // Init the server here.
    thread::spawn(move || {
        let listener = TcpListener::bind("127.0.0.1:7878").unwrap();

        for stream in listener.incoming() {
            let stream = stream.unwrap();

            tcp_sender.send(stream).unwrap();
        }
    });

    let mut handlebars = Handlebars::new();

    handlebars
        .register_template_file("template", "./index.hbs")
        .unwrap();

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

            // Catch the TCP Connection
            while let Ok(mut stream) = tcp_receiver.try_recv() {
                let buf_reader = BufReader::new(&mut stream);
                let http_request: Vec<_> = buf_reader
                    .lines()
                    .map(|result| result.unwrap())
                    .take_while(|line| !line.is_empty())
                    .collect();

                let status_line = "HTTP/1.1 200 OK";
                let contents = handlebars.render("template", &json!({"log_output": &stdio_handler.total_string})).unwrap();
                let length = contents.len();

                let response =
                    format!("{status_line}\r\nContent-Length: {length}\r\n\r\n{contents}");

                stream.write_all(response.as_bytes()).unwrap();
            }
        }

        // Lets get some whitespace.
        println!("\n\n\n\n\n\n\n\n");

        // Set up for the idle server to run.

        'idle: loop {

        }
    }
}
