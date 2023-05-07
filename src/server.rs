use std::io::{BufRead, BufReader, Read, Write};
use serde_json::{json, Map, Value};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, mpsc};
use std::sync::mpsc::Receiver;
use std::thread;
use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod, SslStream};

pub struct Server {
    pub tcp_receiver: Receiver<TcpStream>,
    pub acceptor: Arc<SslAcceptor>
}

impl Server {
    pub fn new(threads: u32, addr: &str) -> Self {
        let mut acceptor = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
        acceptor.set_private_key_file("key.pem", SslFiletype::PEM).unwrap();
        acceptor.set_certificate_chain_file("cert.pem").unwrap();
        acceptor.check_private_key().unwrap();
        let acceptor = Arc::new(acceptor.build());

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

                    // Attempt to send the tcp_stream to the main thread
                    match tcp_sender.send(stream) {
                        Ok(_) => {}
                        // Ignore the error here because the server is most
                        // likely closed.
                        Err(_) => { break; }
                    }
                }
            });
        }

        // Return the server after everything is done running.
        Self {
            tcp_receiver: receiver,
            acceptor
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
    pub tcp_stream: SslStream<TcpStream>,
    pub request: String,
    headers: Map<String, Value>,
    cookies: Map<String, Value>,
    content: Map<String, Value>
}

pub fn parse_stream(stream: TcpStream, server: &Server) -> Result<ServerStream, String> {
    let mut stream = match server.acceptor.accept(stream.try_clone().unwrap()) {
        Ok(str) => str,
        Err(err) => { return Err(err.to_string()); }
    };

    let mut buf_reader = BufReader::new(&mut stream);

    let mut http_request = vec![];
    let mut header_line = "".to_string();

    loop {
        match buf_reader.read_line(&mut header_line) {
            Ok(_) => {},
            Err(err) => { return Err(err.to_string()); }
        };

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

    // Try to get the content
    let mut content = Map::new();

    if let Some(content_length) = headers.get("Content-Length") {
        let content_length = content_length.as_str().unwrap().parse::<usize>().unwrap();

        let mut read_buf = vec![0u8; content_length];
        buf_reader.read_exact(&mut read_buf).unwrap();

        let body = String::from_utf8(read_buf.to_vec()).unwrap();

        let split_body: Vec<_> = body.trim().split("\r\n").collect();

        for piece in split_body {
            let pieces: (&str, &str) = piece.split_once("=").unwrap();

            content.insert(pieces.0.to_string(), json!(pieces.1.to_string()));
        }
    }

    Ok(ServerStream {
        tcp_stream: stream,
        request,
        headers,
        cookies,
        content
    })
}

impl ServerStream {
    pub fn get_request(&self) -> String {
        self.request.clone()
    }

    pub fn get_header(&self, key: &str) -> Option<String> {
        if let Some(value) = self.headers.get(key) {
            if let Some(str_value) = value.as_str() {
                return Some(str_value.to_string());
            }
        }
        None
    }

    pub fn get_cookie(&self, key: &str) -> Option<String> {
        if let Some(value) = self.cookies.get(key) {
            if let Some(str_value) = value.as_str() {
                return Some(str_value.to_string());
            }
        }
        None
    }

    pub fn get_content(&self, key: &str) -> Option<String> {
        if let Some(value) = self.content.get(key) {
            if let Some(str_value) = value.as_str() {
                return Some(str_value.to_string());
            }
        }
        None
    }

    pub fn write_request(mut self, status_line: &str, contents: &str, headers: Vec<&str>) {
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