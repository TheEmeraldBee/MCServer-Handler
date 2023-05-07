use std::process::{Command, Stdio};
use std::{fs, thread};
use serde_json::{json};
use std::time::Duration;
use handlebars::{Handlebars};
use crate::command_watcher::CommandWatcher;
use crate::io_handler::ServerIOHandler;
use crate::server::{parse_stream, Server};
use serde::Deserialize;

use rand::Rng;

pub mod command_watcher;
pub mod io_handler;
pub mod server;

#[derive(Deserialize)]
pub struct Config {
    pub main_user: String,
    pub main_pass: String,
    pub start_user: String,
    pub start_pass: String,
    pub run_path: String,
    pub host_ip: String,
    pub receive_threads: u32,
    pub max_lines_shown: usize
}

fn main() {
    // Generate a random number as a session value.
    let mut session_gen = rand::thread_rng();

    let mut connected_sessions: Vec<String> = vec![];

    let config_file = match fs::read_to_string("mcserver-handler.toml") {
        Ok(file) => file,
        Err(_) => panic!("Please create a mcserver-handler.toml!")
    };
    let config = toml::from_str::<Config>(&config_file).unwrap();

    let main_user = config.main_user;
    let main_pass = config.main_pass;
    let start_user = config.start_user;
    let start_pass = config.start_pass;

    let mut server = Server::new(config.receive_threads, &config.host_ip);

    let mut handlebars = Handlebars::new();

    handlebars
        .register_template_file("template", "./console.hbs")
        .unwrap();

    handlebars
        .register_template_file("login", "./login.hbs")
        .unwrap();

    handlebars
        .register_template_file("offline", "./offline_console.hbs")
        .unwrap();

    handlebars
        .register_template_file("404", "./404.hbs")
        .unwrap();

    'server: loop {
        // Create and start our server.sh file.
        let mut command = Command::new(config.run_path.clone())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        // First build our STDIO Handler that will handle input and console output.
        let mut stdio_handler = ServerIOHandler::new(command.stdout.take().unwrap(), config.max_lines_shown);

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
            for request in server.get_streams() {
                let request = match parse_stream(request, &server) {
                    Ok(req) => { req },
                    Err(_) => { continue }
                };

                match request.get_request().as_str() {
                    "POST /console HTTP/1.1" => {
                        // Check for logged in
                        if connected_sessions.contains(&request.get_cookie("login").unwrap_or("".to_string())) {
                            // They are logged in, so run the command and return a move to the GET /console
                            if let Some(command) = request.get_content("command") {
                                command_watcher.send_string(command).unwrap();
                            }

                            request.write_request("HTTP/1.1 303 See Other", "", vec!["Location: /console"])
                        }
                        // If not logged in, send back to login page
                        else {
                            request.write_request("HTTP/1.1 303 See Other", "", vec!["Location: /"])
                        }
                    }
                    "GET /console HTTP/1.1" => {
                        // Check for logged in
                        if connected_sessions.contains(&request.get_cookie("login").unwrap_or("".to_string())) {
                            // They are logged in, so send them the console page.
                            let contents = handlebars.render("template",
                                                             &json!({"log_output": &stdio_handler.total_string, "user": main_user})).unwrap();
                            request.write_request("HTTP/1.1 200 OK", &contents, vec![]);
                        }
                        // If not logged in, send back to login page
                        else {
                            request.write_request("HTTP/1.1 303 See Other", "", vec!["Location: /"])
                        }
                    }
                    "GET / HTTP/1.1" => {
                        // Check for logged in
                        if connected_sessions.contains(&request.get_cookie("login").unwrap_or("".to_string())) {
                            request.write_request("HTTP/1.1 303 See Other", "", vec!["Location: /console"])
                        } else {
                            let contents = handlebars.render("login", &json!({"login_error": "Server Status: Online"})).unwrap();
                            request.write_request("HTTP/1.1 200 OK", &contents, vec![]);
                        }
                    }
                    "POST / HTTP/1.1" => {
                        // Check if username and password are correct.
                        let username = if let Some(username) = request.get_content("username") { username } else { "".to_string() };
                        let password = if let Some(password) = request.get_content("password") { password } else { "".to_string() };

                        if username == main_user && password == main_pass {
                            // Generate a unique login token
                            let login_token = session_gen.gen::<u32>();
                            connected_sessions.push(login_token.to_string());

                            // They should be logged in now.
                            request.write_request("HTTP/1.1 303 See Other",
                                                  "",
                                                  vec!["Location: /console", &format!("Set-Cookie: login={}; SameSite=Strict; Max-Age=86400", login_token)]);
                        }
                        else if username == start_user && password == start_pass {
                            // Someone wants to start the server, but it is already running.
                            let contents = handlebars.render("login", &json!({"login_error": "Server is already running"})).unwrap();
                            request.write_request("HTTP/1.1 200 OK", &contents, vec![]);
                        }
                        else {
                            let contents = handlebars.render("login", &json!({"login_error": "Username or Password Is Incorrect"})).unwrap();
                            request.write_request("HTTP/1.1 200 OK", &contents, vec![]);
                        }
                    }
                    "GET /data HTTP/1.1" => {
                        // Check for logged in
                        if connected_sessions.contains(&request.get_cookie("login").unwrap_or("".to_string())) {
                            let contents = stdio_handler.total_string.clone();
                            request.write_request("HTTP/1.1 200 OK", &contents.join("\n"), vec![])
                        } else {
                            let contents = "User not logged in.";
                            request.write_request("HTTP/1.1 200 OK", contents, vec![])
                        }
                    }
                    "GET /logout HTTP/1.1" => {
                        // They should be logged out now.

                        // Remove their session from the system
                        connected_sessions.retain(|x| x != &request.get_content("login").unwrap_or("".to_string()));

                        // Remove their login cookie, and return them to the login page.
                        request.write_request("HTTP/1.1 303 See Other",
                                              "",
                                              vec!["Location: /", "Set-Cookie: login=0; SameSite=Strict; Max-Age=-1"]
                        );
                    }
                    "GET /stop HTTP/1.1" => {
                        // Ensure Login, and if so, stop the server
                        // Check for logged in
                        if connected_sessions.contains(&request.get_cookie("login").unwrap_or("".to_string())) {
                            // Stop the server.
                            command_watcher.send_string("stop".to_string()).unwrap();

                            request.write_request("HTTP/1.1 303 See Other", "", vec!["Location: /console"]);

                            break 'command;
                        }
                        // If not logged in, send back to login page
                        else {
                            request.write_request("HTTP/1.1 303 See Other", "", vec!["Location: /"])
                        }
                    }
                    "GET /kill HTTP/1.1" => {
                        // Ensure Login, and if so, kill the server
                        // Ensure Login, and if so, stop the server
                        // Check for logged in
                        if connected_sessions.contains(&request.get_cookie("login").unwrap_or("".to_string())) {
                            // Stop the server.
                            command_watcher.send_string("stop".to_string()).unwrap();

                            request.write_request("HTTP/1.1 303 See Other", "", vec!["Location: /console"]);

                            // Now kill the webserver too.
                            break 'server;
                        }
                        // If not logged in, send back to login page
                        else {
                            request.write_request("HTTP/1.1 303 See Other", "", vec!["Location: /"])
                        }
                    }
                    _ => {
                        let contents = handlebars.render("404", &json!({})).unwrap();
                        request.write_request("HTTP/1.1 404 not found", &contents, vec![])
                    }
                }
            }
        }

        // Lets get some whitespace.
        println!("\n\n-------------------------------------\n\n");

        // Set up for the idle server to run.

        'idle: loop {
            // Catch the TCP Connection
            for request in server.get_streams() {
                let request = match parse_stream(request, &server) {
                    Ok(req) => req,
                    Err(_) => {continue}
                };

                match request.get_request().as_str() {
                    "GET /console HTTP/1.1" => {
                        // Check for logged in
                        if connected_sessions.contains(&request.get_cookie("login").unwrap_or("".to_string())) {
                            // They are logged in, so send them the console page.
                            let contents = handlebars.render("offline", &json!({})).unwrap();
                            request.write_request("HTTP/1.1 200 OK", &contents, vec![]);
                        }
                        // If not logged in, send back to login page
                        else {
                            request.write_request("HTTP/1.1 303 See Other", "", vec!["Location: /"])
                        }
                    }
                    "GET / HTTP/1.1" => {
                        // Send them the login page
                        if connected_sessions.contains(&request.get_cookie("login").unwrap_or("".to_string())) {
                            request.write_request("HTTP/1.1 303 See Other", "", vec!["Location: /console"])
                        } else {
                            let contents = handlebars.render("login", &json!({"login_error": "Server Status: Offline"})).unwrap();
                            request.write_request("HTTP/1.1 200 OK", &contents, vec![]);
                        }
                    }
                    "POST / HTTP/1.1" => {
                        // Check if username and password are correct.
                        let username = if let Some(username) = request.get_content("username") { username } else { "".to_string() };
                        let password = if let Some(password) = request.get_content("password") { password } else { "".to_string() };

                        if username == main_user && password == main_pass {
                            // Generate a unique login token
                            let login_token = session_gen.gen::<u32>();
                            connected_sessions.push(login_token.to_string());

                            // They should be logged in now.
                            request.write_request("HTTP/1.1 303 See Other",
                                                  "",
                                                  vec!["Location: /console", &format!("Set-Cookie: login={}; SameSite=Strict; Max-Age=86400", login_token)]
                            );
                        }
                        else if username == start_user && password == start_pass {
                            // Someone wants to start the server, so run it!
                            let contents = handlebars.render("login", &json!({"login_error": "Server is starting"})).unwrap();
                            request.write_request("HTTP/1.1 200 OK", &contents, vec![]);
                            break 'idle;
                        }
                        else {
                            let contents = handlebars.render("login", &json!({"login_error": "Username or Password Is Incorrect"})).unwrap();
                            request.write_request("HTTP/1.1 200 OK", &contents, vec![]);
                        }
                    }
                    "GET /kill HTTP/1.1" => {
                        // Ensure Login, and if so, kill the server
                        // Ensure Login, and if so, stop the server
                        // Check for logged in
                        if connected_sessions.contains(&request.get_cookie("login").unwrap_or("".to_string())) {
                            request.write_request("HTTP/1.1 303 See Other", "", vec!["Location: /console"]);

                            // Now kill the webserver too.
                            break 'server;
                        }
                        // If not logged in, send back to login page
                        else {
                            request.write_request("HTTP/1.1 303 See Other", "", vec!["Location: /"])
                        }
                    }
                    "GET /start HTTP/1.1" => {
                        if connected_sessions.contains(&request.get_cookie("login").unwrap_or("".to_string())) {
                            request.write_request("HTTP/1.1 303 See Other", "", vec!["Location: /console"]);
                            break 'idle;
                        } else {
                            request.write_request("HTTP/1.1 303 See Other", "", vec!["Location: /"]);
                        }
                    }
                    "GET /logout HTTP/1.1" => {
                        // They should be logged out now.

                        // Remove their session from the system
                        connected_sessions.retain(|x| x != &request.get_content("login").unwrap_or("".to_string()));

                        // Remove their login cookie, and return them to the login page.
                        request.write_request("HTTP/1.1 303 See Other",
                                              "",
                                              vec!["Location: /", "Set-Cookie: login=0; SameSite=Strict; Max-Age=-1"]
                        );
                    }
                    "GET /data HTTP/1.1" => {
                        if connected_sessions.contains(&request.get_cookie("login").unwrap_or("".to_string())) {
                            request.write_request("HTTP/1.1 200 OK", "Server Offline", vec![]);
                        } else {
                            request.write_request("HTTP/1.1 303 See Other", "", vec!["Location: /"]);
                        }
                    }
                    _ => {
                        let contents = handlebars.render("404", &json!({})).unwrap();
                        request.write_request("HTTP/1.1 404 not found", &contents, vec![])
                    }
                }
            }
        }
    }
}
