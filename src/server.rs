use std::io::{BufRead, BufReader, Write};
use serde_json::{json, Map, Value};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::thread;

pub struct Server {
    pub tcp_receiver: Receiver<TcpStream>,
}

impl Server {
    pub fn new(threads: u32, addr: String) -> Self {
        // Init the MPSC sender and receiver.
        let (sender, receiver) = mpsc::channel();

        // Create the server listener
        let listener = TcpListener::bind(addr).unwrap();

        // Init the handler threads.
        for _ in 0..threads {
            let tcp_listener = listener.try_clone().unwrap();
            let tcp_sender = sender.clone();
            // Init the server here.
            thread::spawn(move || {

                for stream in tcp_listener.incoming() {
                    let stream = stream.unwrap();

                    // TODO: Handle when the server is closed.
                    tcp_sender.send(stream).unwrap();
                }
            });
        }

        // Return the server handler.
        Self {
            tcp_receiver: receiver
        }

    }

    pub fn get_streams(&mut self) -> Vec<TcpStream> {
        let mut streams = vec![];

        while let Ok(stream) = self.tcp_receiver.try_recv() {
            streams.push(stream);
        }

        streams
    }
}

pub struct ServerStream {
    pub tcp_stream: TcpStream,
    pub request: String,
    headers: Map<String, Value>,
    cookies: Map<String, Value>
}

pub fn parse_stream(mut stream: TcpStream) -> ServerStream {
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

    let mut headers = Map::new();
    for request in &http_request {
        let split: Vec<_> = request.trim().split(": ").collect();

        headers.insert(split[0].to_string().clone(), json!(split[1].to_string().clone()));
    }

    let mut cookies = Map::new();
    // Check for logged in
    if let Some(cookie_str) = headers.get("Cookie") {
        let cookie_str = cookie_str.as_str().unwrap();
        let split_cookies: Vec<_> = cookie_str.split("; ").collect();

        for cookie_pair in split_cookies {
            let split_pair: (&str, &str) = cookie_pair.split_once("=").unwrap();

            cookies.insert(split_pair.0.to_string(), json!(split_pair.1));
        }
    }

    ServerStream {
        tcp_stream: stream,
        request,
        headers,
        cookies
    }
}

impl ServerStream {
    pub fn get_request(&self) -> String {
        self.request.clone()
    }

    pub fn get_header(&self, key: String) -> Option<String> {
        if let Some(value) = self.headers.get(&key) {
            if let Some(str_value) = value.as_str() {
                return Some(str_value.to_string());
            }
        }
        None
    }

    pub fn get_cookie(&self, key: String) -> Option<String> {
        if let Some(value) = self.cookies.get(&key) {
            if let Some(str_value) = value.as_str() {
                return Some(str_value.to_string());
            }
        }
        None
    }

    pub fn write_request(mut self, status_line: String, contents: String, headers: Vec<String>) {
        let mut response = String::new();

        // Build the headers
        response.push_str(&format!("{status_line}\r\n"));

        for header in &headers {
            response.push_str(&format!("{header}\r\n"));
        }

        let length = contents.len();

        response.push_str(&format!("Content-Length: {length}\r\n\r\n{contents}"));

        self.tcp_stream.write_all(response.as_bytes()).unwrap();
    }
}