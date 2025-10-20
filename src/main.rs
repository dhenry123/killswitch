// authors dhenry for mytinydc.com
// Licence MIT

use clap::Parser;
use std::io::prelude::*;
use std::net::TcpListener;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use chrono::Utc;
use rand::Rng;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Listening port
    #[arg(short, long, default_value_t = 8000)]
    port: u16,

    /// First secret
    #[arg(short, long)]
    first_secret: String,

    /// File path for first shell hook (kill operation)
    #[arg(short = 'k', long)]
    kill_hook: String,

    /// File path for second shell hook (restore operation)
    #[arg(short = 'r', long)]
    restore_hook: String,

    /// Delay in seconds between kill and restore operations
    #[arg(short = 'd', long, default_value_t = 300)] // 5 minutes default
    restore_delay: u64,
}

struct KillswitchServer {
    listener: TcpListener,
    first_secret: String,
    kill_hook: String,
    restore_hook: String,
    restore_delay: u64,
    random_words_list: Arc<Mutex<Vec<String>>>,
}

impl KillswitchServer {
    fn new(args: Args) -> Result<Self, std::io::Error> {
        let listener = TcpListener::bind(format!("0.0.0.0:{}", args.port))?;

        Ok(Self {
            listener,
            first_secret: args.first_secret,
            kill_hook: args.kill_hook,
            restore_hook: args.restore_hook,
            restore_delay: args.restore_delay,
            random_words_list: Arc::new(Mutex::new(Vec::new())),
        })
    }

    fn run(&self) {
        log(&format!(
            "Server running on http://0.0.0.0:{}",
            self.listener.local_addr().unwrap().port()
        ));

        for stream in self.listener.incoming() {
            match stream {
                Ok(stream) => {
                    self.handle_connection(stream);
                }
                Err(e) => {
                    log(&format!("Connection failed: {}", e));
                }
            }
        }
    }

    fn handle_connection(&self, stream: std::net::TcpStream) {
        let first_secret = self.first_secret.clone();
        let kill_hook = self.kill_hook.clone();
        let restore_hook = self.restore_hook.clone();
        let restore_delay = self.restore_delay.clone();
        let random_words_list = self.random_words_list.clone();

        thread::spawn(move || {
            let mut stream = stream;
            let mut buffer = [0; 1024];

            if let Ok(size) = stream.read(&mut buffer) {
                let request = String::from_utf8_lossy(&buffer[..size]);

                if let Err(e) = Self::process_request(
                    &mut stream,
                    &request,
                    &first_secret,
                    &kill_hook,
                    &restore_hook,
                    &restore_delay,
                    &random_words_list,
                ) {
                    log(&format!("Error processing request: {}", e));
                }
            }
        });
    }

    fn process_request(
        stream: &mut std::net::TcpStream,
        request: &str,
        first_secret: &str,
        kill_hook: &str,
        restore_hook: &str,
        restore_delay: &u64,
        random_words_list: &Arc<Mutex<Vec<String>>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // First check if it's the first secret
        if request.contains(first_secret) {
            let mut list_locked = random_words_list
                .lock()
                .map_err(|e| format!("Mutex lock error: {}", e))?;
            Self::handle_first_secret(stream, &mut list_locked)?;
        }
        // Then check for second secrets - we need to find which secret matches first
        else {
            // First, find which secret matches (if any) without holding the lock for too long
            let matched_secret = {
                let list_locked = random_words_list
                    .lock()
                    .map_err(|e| format!("Mutex lock error: {}", e))?;
                list_locked
                    .iter()
                    .find(|secret| request.contains(secret.as_str()))
                    .map(|s| s.clone()) // Clone the secret to use outside the lock
            };

            if let Some(secret) = matched_secret {
                // Now we can process the second secret with a mutable lock
                let mut list_locked = random_words_list
                    .lock()
                    .map_err(|e| format!("Mutex lock error: {}", e))?;
                Self::handle_second_secret(
                    stream,
                    &secret,
                    kill_hook,
                    restore_hook,
                    restore_delay,
                    &mut list_locked,
                )?;
            } else {
                Self::handle_invalid_request(stream)?;
            }
        }

        Ok(())
    }

    fn handle_first_secret(
        stream: &mut std::net::TcpStream,
        secrets_list: &mut Vec<String>,
    ) -> Result<(), std::io::Error> {
        log("Killswitch 1st secret: OK - Waiting for the 2nd secret to start process");

        let random_word = generate_random_word();
        let response = format!("HTTP/1.1 200 OK\r\n\r\n{}", &random_word);

        stream.write_all(response.as_bytes())?;
        stream.flush()?;

        secrets_list.push(random_word);
        Ok(())
    }

    fn handle_second_secret(
        stream: &mut std::net::TcpStream,
        secret: &str,
        kill_hook: &str,
        restore_hook: &str,
        restore_delay: &u64,
        secrets_list: &mut Vec<String>,
    ) -> Result<(), std::io::Error> {
        log("Killswitch 2nd secret: OK - starting kill switch process");

        let response = "HTTP/1.1 200 OK\r\n\r\nKillswitch is started\n";
        stream.write_all(response.as_bytes())?;
        stream.flush()?;

        // Remove the used secret
        if let Some(index) = secrets_list.iter().position(|s| s == secret) {
            secrets_list.remove(index);
        }

        // Execute kill hook immediately
        Self::execute_hook(kill_hook, "kill");

        // Schedule restore hook after 5 minutes
        let restore_hook_clone = restore_hook.to_string();
        let restore_delay_clone = restore_delay.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_secs(restore_delay_clone));
            Self::execute_hook(&restore_hook_clone, "restore");
        });

        Ok(())
    }

    fn handle_invalid_request(stream: &mut std::net::TcpStream) -> Result<(), std::io::Error> {
        log("Url path not found or secret is not valid");
        let response =
            "HTTP/1.1 404 Not Found\r\n\r\n404 - Url path not found or secret is not valid\n";
        stream.write_all(response.as_bytes())?;
        Ok(())
    }

    fn execute_hook(hook_path: &str, hook_type: &str) {
        log(&format!("Executing {} hook: {}", hook_type, hook_path));

        match Command::new("sh").arg("-c").arg(hook_path).output() {
            Ok(output) => {
                if output.status.success() {
                    log(&format!("{} hook executed successfully", hook_type));
                } else {
                    log(&format!(
                        "{} hook failed with status: {:?}",
                        hook_type, output.status
                    ));
                }
            }
            Err(e) => {
                log(&format!("Error executing {} hook: {}", hook_type, e));
            }
        }
    }
}

fn generate_random_word() -> String {
    let chars: Vec<char> = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"
        .chars()
        .collect();
    (0..12)
        .map(|_| chars[rand::rng().random_range(0..chars.len())])
        .collect()
}

fn log(message: &str) {
    println!("{} \"{}\"", Utc::now(), message);
}

fn main() {
    let args = Args::parse();

    match KillswitchServer::new(args) {
        Ok(server) => server.run(),
        Err(e) => {
            eprintln!("Failed to start server: {}", e);
            std::process::exit(1);
        }
    }
}
