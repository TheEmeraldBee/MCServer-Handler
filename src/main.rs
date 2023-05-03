use std::process::{Command, Stdio};
use std::{thread};
use std::io::{BufRead, BufReader, Read, Write};
use serde_json::{json, Map};
use std::net::TcpListener;
use std::sync::mpsc;
use std::time::Duration;
use handlebars::{Handlebars};
use crate::command_watcher::CommandWatcher;
use crate::io_handler::ServerIOHandler;

pub mod command_watcher;
pub mod io_handler;
pub mod server;

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

    handlebars
        .register_template_file("login", "./login.hbs")
        .unwrap();

    handlebars
        .register_template_file("404", "./404.hbs")
        .unwrap();

    loop {
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
                let mut buf_reader = BufReader::new(&mut stream);

                let mut http_request = vec![];
                let mut header_line = "".to_string();

                loop {
                    buf_reader.read_line(&mut header_line).unwrap();

                    // The final line is just /r/n
                    if header_line.len() == 2 {
                        break
                    }

                    http_request.push(header_line);

                    header_line = "".to_string();
                }

                let mut request = http_request[0].clone();
                request = request.trim().to_string();
                http_request.remove(0);

                let mut data_map = Map::new();
                for request in &http_request {
                    let split: Vec<_> = request.trim().split(": ").collect();

                    data_map.insert(split[0].to_string().clone(), json!(split[1].to_string().clone()));
                }

                let contents;
                let status_line;
                let mut extra = String::new();

                if request == "POST /console HTTP/1.1" {
                    // Check for logged in
                    if let Some(cookies) = data_map.get("Cookie") {

                        let mut cookie_map = Map::new();

                        let cookies = cookies.as_str().unwrap();
                        let split_cookies: Vec<_> = cookies.split("; ").collect();

                        for cookie_pair in split_cookies {
                            let split_pair: Vec<_> = cookie_pair.split("=").collect();

                            cookie_map.insert(split_pair[0].to_string(), json!(split_pair[1]));
                        }

                        if let Some(cookie) = cookie_map.get("login") {
                            let cookie = cookie.as_str().unwrap();

                            if cookie == "129248921" {
                                let content_length = data_map["Content-Length"].to_string().replace("\"", "").parse::<usize>().unwrap();

                                let mut read_buf = vec![0u8; content_length];
                                buf_reader.read_exact(&mut read_buf).unwrap();

                                let body = String::from_utf8(read_buf.to_vec()).unwrap();

                                let split_body: Vec<_> = body.trim().split("\n").collect();

                                let mut body_map = Map::new();

                                for piece in split_body {
                                    let pieces: (&str, &str) = piece.split_once("=").unwrap();

                                    body_map.insert(pieces.0.to_string(), json!(pieces.1.to_string()));
                                }

                                if let Some(command) = body_map.get("command") {
                                    let mut command = command.as_str().unwrap().to_string();
                                    command.push('\n');

                                    command_watcher.send_string(command).unwrap();
                                }

                                status_line = "HTTP/1.1 303 See Other";
                                extra = "Location: /console".to_string();
                                contents = "".to_string();
                            } else {
                                status_line = "HTTP/1.1 303 See Other";
                                extra = "Location: /".to_string();
                                contents = "".to_string();
                            }
                        } else {
                            status_line = "HTTP/1.1 303 See Other";
                            extra = "Location: /".to_string();
                            contents = "".to_string();
                        }
                    } else {
                        status_line = "HTTP/1.1 303 See Other";
                        extra = "Location: /".to_string();
                        contents = "".to_string();
                    }
                } else if request == "GET /console HTTP/1.1" {
                    // Check for logged in
                    if let Some(cookies) = data_map.get("Cookie") {

                        let mut cookie_map = Map::new();

                        let cookies = cookies.as_str().unwrap();
                        let split_cookies: Vec<_> = cookies.split("; ").collect();

                        for cookie_pair in split_cookies {
                            let split_pair: Vec<_> = cookie_pair.split("=").collect();

                            cookie_map.insert(split_pair[0].to_string(), json!(split_pair[1]));
                        }

                        if let Some(cookie) = cookie_map.get("login") {
                            let cookie = cookie.as_str().unwrap();

                            if cookie == "129248921" {
                                status_line = "HTTP/1.1 200 OK";
                                contents = handlebars.render("template", &json!({"log_output": &stdio_handler.total_string, "user": "Emerald"})).unwrap();
                            } else {
                                status_line = "HTTP/1.1 303 See Other";
                                extra = "Location: /".to_string();
                                contents = "".to_string();
                            }
                        } else {
                            status_line = "HTTP/1.1 303 See Other";
                            extra = "Location: /".to_string();
                            contents = "".to_string();
                        }
                    } else {
                        status_line = "HTTP/1.1 303 See Other";
                        extra = "Location: /".to_string();
                        contents = "".to_string();
                    }
                }
                else if request == "GET / HTTP/1.1" {
                    // Check for logged in
                    if let Some(cookies) = data_map.get("Cookie") {

                        let mut cookie_map = Map::new();

                        let cookies = cookies.as_str().unwrap();
                        let split_cookies: Vec<_> = cookies.split("; ").collect();

                        for cookie_pair in split_cookies {
                            let split_pair: Vec<_> = cookie_pair.split("=").collect();

                            cookie_map.insert(split_pair[0].to_string(), json!(split_pair[1]));
                        }

                        if let Some(cookie) = cookie_map.get("login") {
                            let cookie = cookie.as_str().unwrap();

                            if cookie == "129248921" {
                                status_line = "HTTP/1.1 303 See Other";
                                extra = "Location: /console".to_string();
                                contents = "".to_string();
                            } else {
                                status_line = "HTTP/1.1 200 OK";
                                contents = handlebars.render("login", &json!({})).unwrap();
                            }
                        } else {
                            status_line = "HTTP/1.1 200 OK";
                            contents = handlebars.render("login", &json!({})).unwrap();
                        }


                    } else {
                        status_line = "HTTP/1.1 200 OK";
                        contents = handlebars.render("login", &json!({})).unwrap();
                    }
                } else if request == "POST / HTTP/1.1" {
                    let content_length = data_map["Content-Length"].to_string().replace("\"", "").parse::<usize>().unwrap();

                    let mut read_buf = vec![0u8; content_length];
                    buf_reader.read_exact(&mut read_buf).unwrap();

                    let body = String::from_utf8(read_buf.to_vec()).unwrap();

                    let split_body: Vec<_> = body.trim().split("&").collect();

                    let mut body_map = Map::new();

                    for piece in split_body {
                        let pieces: Vec<_> = piece.split("=").collect();

                        body_map.insert(pieces[0].to_string(), json!(pieces[1].to_string()));
                    }

                    let username = body_map["username"].as_str().unwrap();
                    let pass = body_map["password"].as_str().unwrap();

                    if username == "Emerald" && pass == "breaktheworld" {
                        status_line = "HTTP/1.1 303 See Other";
                        contents = "".to_string();
                        extra = "Location: /console\r\nSet-Cookie: login=129248921; SameSite=Strict; Max-Age=86400".to_string();
                    } else if username == "" && pass == "" {
                        status_line = "HTTP/1.1 200 OK";
                        contents = handlebars.render("login", &json!({"login_error": "Server already running."})).unwrap();
                    } else {
                        status_line = "HTTP/1.1 200 OK";
                        contents = handlebars.render("login", &json!({"login_error": "Username or Password Incorrect, Try Again"})).unwrap();
                    }
                } else if request == "GET /data HTTP/1.1" {
                    // Check for logged in
                    if let Some(cookies) = data_map.get("Cookie") {

                        let mut cookie_map = Map::new();

                        let cookies = cookies.as_str().unwrap();
                        let split_cookies: Vec<_> = cookies.split("; ").collect();

                        for cookie_pair in split_cookies {
                            let split_pair: Vec<_> = cookie_pair.split("=").collect();

                            cookie_map.insert(split_pair[0].to_string(), json!(split_pair[1]));
                        }

                        if let Some(cookie) = cookie_map.get("login") {
                            let cookie = cookie.as_str().unwrap();

                            if cookie == "129248921" {
                                status_line = "HTTP/1.1 200 OK";
                                contents = stdio_handler.total_string.clone();
                            } else {
                                status_line = "HTTP/1.1 303 See Other";
                                extra = "Location: /".to_string();
                                contents = "".to_string();
                            }
                        } else {
                            status_line = "HTTP/1.1 303 See Other";
                            extra = "Location: /".to_string();
                            contents = "".to_string();
                        }
                    } else {
                        status_line = "HTTP/1.1 303 See Other";
                        extra = "Location: /".to_string();
                        contents = "".to_string();
                    }
                } else if request == "GET /logout HTTP/1.1" {
                    status_line = "HTTP/1.1 303 See Other";
                    extra = "Location: /\r\nSet-Cookie: login=0; SameSite=Strict; Max-Age=-1".to_string();
                    contents = "".to_string();
                } else {
                    status_line = "HTTP/1.1 404 not found";
                    contents = handlebars.render("404", &json!({})).unwrap();
                }

                let length = contents.len();

                if extra != "".to_string() {
                    extra.push_str("\r\n");
                }

                let response =
                    format!("{status_line}\r\n{extra}Content-Length: {length}\r\n\r\n{contents}");

                stream.write_all(response.as_bytes()).unwrap();
            }
        }

        // Lets get some whitespace.
        println!("\n\n");

        // Set up for the idle server to run.

        'idle: loop {

        }
    }
}
