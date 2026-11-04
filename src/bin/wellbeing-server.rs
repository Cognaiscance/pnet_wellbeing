//! Stores client reports on the top-ranked server and shows them on the portal.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use pnet_wellbeing::directory::{self, Directory};
use pnet_wellbeing::node::{self, NodeClient};
use pnet_wellbeing::page::{self, http_response};
use pnet_wellbeing::proto::{self, Message};
use pnet_wellbeing::store::{self, Report, Store};
use pnet_wellbeing::{DEFAULT_HTTP_PORT, DEFAULT_SERVER_PUSH, SERVER_ALIAS, WEB_SLUG, WEB_TITLE};

fn main() {
    let addr = env_or("PNET_ADDR", "127.0.0.1:7777");
    let addr: SocketAddr = addr.parse().unwrap_or_else(|_| {
        eprintln!("[wellbeing-server] PNET_ADDR is not host:port");
        std::process::exit(1);
    });
    let push_port = env_or("WELLBEING_PUSH_PORT", &DEFAULT_SERVER_PUSH.to_string())
        .parse()
        .unwrap_or(DEFAULT_SERVER_PUSH);
    let http_port = env_or("WELLBEING_HTTP_PORT", &DEFAULT_HTTP_PORT.to_string())
        .parse()
        .unwrap_or(DEFAULT_HTTP_PORT);
    let portal = env_or("PNET_PORTAL", "http://127.0.0.1:8777");

    let push = node::bind_push(push_port).unwrap_or_else(|err| {
        eprintln!("[wellbeing-server] bind 127.0.0.1:{push_port}: {err}");
        std::process::exit(1);
    });
    let node = NodeClient::connect(addr).unwrap_or_else(|err| {
        eprintln!("[wellbeing-server] local node {addr}: {err}");
        std::process::exit(1);
    });
    let token_path = node::token_path("wellbeing-server");
    let token = match node.register(SERVER_ALIAS, push_port) {
        Ok(token) => {
            if let Err(err) = node::save_token(&token_path, &token) {
                eprintln!("[wellbeing-server] token file: {err}");
            }
            println!("[wellbeing-server] registered as {SERVER_ALIAS}");
            token
        }
        Err(err) => {
            eprintln!("[wellbeing-server] register failed: {err}");
            std::process::exit(1);
        }
    };

    loop {
        match fetch(&node, &token) {
            Ok(dir) if directory::holds_report(&dir) => {
                println!("[wellbeing-server] this is the top-ranked SG; storing reports");
                serve(&node, &push, &token, http_port, &portal, &dir.owner_alias);
                println!("[wellbeing-server] standing by; a higher-ranked SG holds the report");
            }
            Ok(dir) => {
                let holder = directory::report_holder(&dir)
                    .map(|device| device.alias.as_str())
                    .unwrap_or("none");
                println!("[wellbeing-server] standing by; report holder is {holder}");
            }
            Err(err) => eprintln!("[wellbeing-server] directory: {err}"),
        }
        thread::sleep(Duration::from_secs(15));
    }
}

fn serve(
    node: &NodeClient,
    push: &std::net::UdpSocket,
    token: &[u8; 16],
    http_port: u16,
    portal: &str,
    owner_alias: &str,
) {
    let path = store::results_path(&node::data_dir());
    let store = match Store::open(path) {
        Ok(store) => Arc::new(Mutex::new(store)),
        Err(err) => {
            eprintln!("[wellbeing-server] results file: {err}");
            return;
        }
    };
    let stop = Arc::new(AtomicBool::new(false));
    let listener = match TcpListener::bind(("127.0.0.1", http_port)) {
        Ok(listener) => listener,
        Err(err) => {
            eprintln!("[wellbeing-server] http 127.0.0.1:{http_port}: {err}");
            return;
        }
    };
    listener.set_nonblocking(true).ok();
    let owner = Arc::new(Mutex::new(owner_alias.to_string()));
    let page_store = Arc::clone(&store);
    let page_owner = Arc::clone(&owner);
    let page_stop = Arc::clone(&stop);
    thread::spawn(move || http_loop(listener, page_store, page_owner, page_stop));

    let mut mounted = false;
    for attempt in 1..=5 {
        match portal_register(portal, http_port) {
            Ok(()) => {
                println!("[wellbeing-server] portal /apps/{WEB_SLUG}/ → 127.0.0.1:{http_port}");
                mounted = true;
                break;
            }
            Err(err) => {
                eprintln!("[wellbeing-server] portal register attempt {attempt}: {err}");
                thread::sleep(Duration::from_millis(400));
            }
        }
    }

    let mut next_check = Instant::now() + Duration::from_secs(15);
    loop {
        if Instant::now() >= next_check {
            next_check = Instant::now() + Duration::from_secs(15);
            match fetch(node, token) {
                Ok(dir) if !directory::holds_report(&dir) => break,
                Ok(dir) => {
                    *owner.lock().unwrap_or_else(|err| err.into_inner()) = dir.owner_alias;
                }
                Err(err) => eprintln!("[wellbeing-server] directory: {err}"),
            }
        }
        match node::recv_push(push) {
            Ok(Some(datagram)) => {
                if let Err(err) = accept_report(node, token, &store, &owner, &datagram) {
                    eprintln!("[wellbeing-server] {err}");
                }
            }
            Ok(None) => {}
            Err(err) => eprintln!("[wellbeing-server] push: {err}"),
        }
    }

    stop.store(true, Ordering::SeqCst);
    if mounted {
        if let Err(err) = portal_unregister(portal) {
            eprintln!("[wellbeing-server] portal unregister: {err}");
        }
    }
}

fn accept_report(
    node: &NodeClient,
    token: &[u8; 16],
    store: &Mutex<Store>,
    owner: &Mutex<String>,
    datagram: &[u8],
) -> Result<(), String> {
    let Some((sender, payload)) = node::decode_push(datagram) else {
        return Ok(());
    };
    let Message::Report {
        target,
        online,
        delay_ms,
        bytes_per_sec,
        at_ms,
    } = proto::decode(payload).map_err(|err| err.to_string())?
    else {
        return Ok(());
    };
    let dir = fetch(node, token)?;
    *owner.lock().unwrap_or_else(|err| err.into_inner()) = dir.owner_alias.clone();
    if !directory::approved_own_client(&dir, &sender) {
        return Ok(());
    }
    let Some(from_id) = directory::device_of_app(&dir, &sender) else {
        return Ok(());
    };
    let (from_alias, _) = directory::lookup_device(&dir, &from_id)
        .map(|(device, who)| (device.alias.clone(), who))
        .unwrap_or_default();
    let (to_alias, who) = directory::lookup_device(&dir, &target)
        .map(|(device, who)| (device.alias.clone(), who))
        .unwrap_or_else(|| (String::new(), "unknown".to_string()));
    let report = Report {
        from_id,
        from_alias,
        to_id: target,
        to_alias,
        who,
        online,
        delay_ms,
        bytes_per_sec,
        at_ms,
    };
    store
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .upsert(report)
        .map_err(|err| err.to_string())
}

fn http_loop(
    listener: TcpListener,
    store: Arc<Mutex<Store>>,
    owner: Arc<Mutex<String>>,
    stop: Arc<AtomicBool>,
) {
    while !stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => {
                let store = Arc::clone(&store);
                let owner = Arc::clone(&owner);
                thread::spawn(move || {
                    if let Err(err) = serve_http(stream, &store, &owner) {
                        eprintln!("[wellbeing-server] http: {err}");
                    }
                });
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(50));
            }
            Err(err) => {
                eprintln!("[wellbeing-server] accept: {err}");
                thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

fn serve_http(
    mut stream: TcpStream,
    store: &Mutex<Store>,
    owner: &Mutex<String>,
) -> Result<(), String> {
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
    let mut buf = [0u8; 2048];
    let n = stream.read(&mut buf).map_err(|err| err.to_string())?;
    if n == 0 {
        return Ok(());
    }
    let req = String::from_utf8_lossy(&buf[..n]);
    if req.lines().next().unwrap_or("").contains("favicon") {
        let resp = b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
        stream.write_all(resp).ok();
        return Ok(());
    }
    let rows = store
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .rows()
        .to_vec();
    let owner_alias = owner.lock().unwrap_or_else(|err| err.into_inner()).clone();
    stream
        .write_all(&http_response(&page::render(&rows, &owner_alias)))
        .map_err(|err| err.to_string())
}

fn fetch(node: &NodeClient, token: &[u8; 16]) -> Result<Directory, String> {
    let reply = node.get_data(token).map_err(|err| err.to_string())?;
    directory::parse_get_data(&reply).map_err(|err| err.to_string())
}

fn portal_register(portal: &str, port: u16) -> Result<(), String> {
    let body = format!("slug={WEB_SLUG}&port={port}&title={}", form_enc(WEB_TITLE));
    http_post(portal, "/api/app-web/register", &body)
}

fn portal_unregister(portal: &str) -> Result<(), String> {
    http_post(
        portal,
        "/api/app-web/unregister",
        &format!("slug={WEB_SLUG}"),
    )
}

fn http_post(base: &str, path: &str, body: &str) -> Result<(), String> {
    let base = base.trim_end_matches('/');
    let url = format!("{base}{path}");
    let (host, port) = parse_http(&url)?;
    let mut stream = TcpStream::connect((host.as_str(), port)).map_err(|err| err.to_string())?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
    let req = format!(
        "POST {path} HTTP/1.1\r\nHost: {host}:{port}\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(req.as_bytes())
        .map_err(|err| err.to_string())?;
    let mut resp = String::new();
    stream
        .read_to_string(&mut resp)
        .map_err(|err| err.to_string())?;
    if resp.starts_with("HTTP/1.1 200") || resp.starts_with("HTTP/1.0 200") {
        Ok(())
    } else {
        Err(resp.lines().next().unwrap_or("portal error").to_string())
    }
}

fn parse_http(url: &str) -> Result<(String, u16), String> {
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| "portal URL must be http://".to_string())?;
    let (hostport, _) = rest.split_once('/').unwrap_or((rest, ""));
    if let Some((host, port)) = hostport.rsplit_once(':') {
        let port = port.parse().map_err(|_| "bad portal port".to_string())?;
        Ok((host.to_string(), port))
    } else {
        Ok((hostport.to_string(), 80))
    }
}

fn form_enc(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => out.push(byte as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}
