use std::io::{BufRead, BufReader};
use std::process::{ChildStdout};
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::{io, thread};
use std::thread::JoinHandle;

pub struct ServerIOHandler {
    // This is the value that will be sent as the console to the server.
    pub total_string: Vec<String>,

    // This catches the console input from the server machine
    pub stdin_thread: JoinHandle<()>,
    pub stdin_receiver: Receiver<String>,

    // This catches the console output from the child process
    pub stdout_thread: JoinHandle<()>,
    pub stdout_receiver: Receiver<String>,

    // The maximum number of lines that the server will store and show from the underlying process.
    pub max_lines: usize
}

impl ServerIOHandler {
    pub fn new(stdout: ChildStdout, max_lines: usize) -> Self {
        let (in_send, in_receive) = mpsc::channel();
        let (out_send, out_receive) = mpsc::channel();

        let stdin_thread = thread::spawn(move || input_catcher(in_send));

        let stdout_thread = thread::spawn(move || output_catcher(out_send, stdout));

        Self {
            total_string: vec![],

            stdin_thread,
            stdin_receiver: in_receive,

            stdout_thread,
            stdout_receiver: out_receive,

            max_lines
        }
    }

    pub fn handle_output(&mut self) {
        while let Ok(receive) = self.stdout_receiver.try_recv() {
            print!("{receive}");
            self.total_string.push(receive);

            while self.total_string.len() > self.max_lines {
                self.total_string.remove(0);
            }
        }
    }

    pub fn handle_input(&mut self) -> Vec<String> {
        let mut result = vec![];
        // Handle The Input
        while let Ok(receive) = self.stdin_receiver.try_recv() {
            println!("Received {receive}");
            result.push(receive);
        }

        result
    }
}

pub fn output_catcher(msg_link: Sender<String>, stdout: ChildStdout) {
    let mut reader = BufReader::new(stdout);

    loop {
        let mut output = String::new();
        match reader.read_line(&mut output) {
            Ok(_) => { }
            Err(_) => { break; }
        }

        match msg_link.send(output) {
            Ok(_) => {},
            Err(_) => { break; }
        }
    }

    println!("Output catcher done.")
}

pub fn input_catcher(msg_link: Sender<String>) {
    let mut reader = BufReader::new(io::stdin());

    loop {
        let mut output = String::new();
        match reader.read_line(&mut output) {
            Ok(_) => {}
            Err(_) => { break; }
        }

        match msg_link.send(output) {
            Ok(_) => {}
            Err(_) => { break; }
        }
    }

    println!("Input catcher done.")
}