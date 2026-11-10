//! Measures reachability to other wellbeing clients and reports to the top server.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use pnet_wellbeing::directory::{self, Directory, ProbeTarget};
use pnet_wellbeing::node::{self, NodeClient};
use pnet_wellbeing::proto::{self, Message, XFER_CHUNKS};
use pnet_wellbeing::{CLIENT_ALIAS, DEFAULT_CLIENT_PUSH};

fn main() {
    pnet_wellbeing::log_setup::init("wellbeing-client");
    let addr = env_or("PNET_ADDR", "127.0.0.1:7777");
    let addr: SocketAddr = addr.parse().unwrap_or_else(|_| {
        log::error!("[wellbeing-client] PNET_ADDR is not host:port");
        std::process::exit(1);
    });
    let push_port = env_or("WELLBEING_PUSH_PORT", &DEFAULT_CLIENT_PUSH.to_string())
        .parse()
        .unwrap_or(DEFAULT_CLIENT_PUSH);
    let interval = Duration::from_secs(
        env_or("WELLBEING_INTERVAL_SECS", "60")
            .parse::<u64>()
            .unwrap_or(60)
            .max(5),
    );

    let push = node::bind_push(push_port).unwrap_or_else(|err| {
        log::error!("[wellbeing-client] bind 127.0.0.1:{push_port}: {err}");
        std::process::exit(1);
    });
    let node = NodeClient::connect(addr).unwrap_or_else(|err| {
        log::error!("[wellbeing-client] local node {addr}: {err}");
        std::process::exit(1);
    });
    let token_path = node::token_path("wellbeing-client");
    let token = match node.register(CLIENT_ALIAS, push_port) {
        Ok(token) => {
            if let Err(err) = node::save_token(&token_path, &token) {
                log::error!("[wellbeing-client] token file: {err}");
            }
            log::info!("[wellbeing-client] registered as {CLIENT_ALIAS}");
            token
        }
        Err(err) => {
            log::error!("[wellbeing-client] register failed: {err}");
            std::process::exit(1);
        }
    };

    let mut xfers: HashMap<([u8; 16], u64), Vec<u16>> = HashMap::new();
    let mut next = Instant::now();
    let mut seq = 1u64;
    log::info!("[wellbeing-client] checking every {}s", interval.as_secs());

    loop {
        if Instant::now() >= next {
            next = Instant::now() + interval;
            if let Err(err) = run_round(&node, &push, &token, &mut xfers, &mut seq) {
                log::error!("[wellbeing-client] {err}");
            }
        }
        match node::recv_push(&push) {
            Ok(Some(datagram)) => {
                if let Ok(dir) = fetch(&node, &token) {
                    let _ = answer(&node, &token, &dir, &mut xfers, &datagram);
                }
            }
            Ok(None) => {}
            Err(err) => log::error!("[wellbeing-client] push: {err}"),
        }
    }
}

fn run_round(
    node: &NodeClient,
    push: &std::net::UdpSocket,
    token: &[u8; 16],
    xfers: &mut HashMap<([u8; 16], u64), Vec<u16>>,
    seq: &mut u64,
) -> Result<(), String> {
    let dir = fetch(node, token)?;
    if !dir.local_app_approved {
        log::info!("[wellbeing-client] waiting for approval");
        return Ok(());
    }
    let Some(server) = directory::report_server(&dir) else {
        log::info!("[wellbeing-client] no approved server on the top-ranked SG yet");
        return Ok(());
    };
    let targets = directory::probe_targets(&dir);
    if targets.is_empty() {
        log::info!("[wellbeing-client] no other clients to check");
        return Ok(());
    }
    for target in targets {
        let sample = measure(node, push, token, &dir, xfers, seq, &target);
        let payload = proto::encode(&Message::Report {
            target: target.device,
            online: sample.online,
            delay_ms: sample.delay_ms,
            bytes_per_sec: sample.bytes_per_sec,
            at_ms: unix_ms(),
            path: sample.path.clone(),
        });
        let sent = if server.device == dir.local_device {
            deliver_local(&dir.local_app_id, server.push_port, &payload)
        } else {
            node.send(token, &server.device, &server.app, &payload)
                .map(|_| ())
                .map_err(|err| err.to_string())
        };
        match sent {
            Ok(()) => log::info!(
                "[wellbeing-client] {} {}: delay {} ms via {}, {} B/s",
                target.device_alias,
                if sample.online {
                    "online"
                } else {
                    "not online"
                },
                sample.delay_ms,
                sample.path,
                sample.bytes_per_sec
            ),
            Err(err) => log::error!("[wellbeing-client] report to server: {err}"),
        }
    }
    Ok(())
}

struct Sample {
    online: bool,
    delay_ms: u32,
    bytes_per_sec: u32,
    path: String,
}

fn measure(
    node: &NodeClient,
    push: &std::net::UdpSocket,
    token: &[u8; 16],
    dir: &Directory,
    xfers: &mut HashMap<([u8; 16], u64), Vec<u16>>,
    seq: &mut u64,
    target: &ProbeTarget,
) -> Sample {
    let ping_id = next_id(seq);
    let ping = proto::encode(&Message::Ping { id: ping_id });
    let path = match node.send(token, &target.device, &target.app, &ping) {
        Ok(path) => path.to_string(),
        Err(err) => {
            log::error!("[wellbeing-client] ping {}: {err}", target.device_alias);
            return Sample {
                online: false,
                delay_ms: 0,
                bytes_per_sec: 0,
                path: "unknown".into(),
            };
        }
    };
    let started = Instant::now();
    let pong = wait_until(
        node,
        push,
        token,
        dir,
        xfers,
        proto::PING_WAIT,
        |msg| matches!(msg, Message::Pong { id } if *id == ping_id),
    );
    if pong.is_none() {
        return Sample {
            online: false,
            delay_ms: 0,
            bytes_per_sec: 0,
            path,
        };
    }
    let delay_ms = proto::delay_ms(started.elapsed());

    let xfer_id = next_id(seq);
    let xfer_started = Instant::now();
    for index in 0..XFER_CHUNKS {
        let chunk = proto::encode(&Message::Xfer {
            id: xfer_id,
            index,
            total: XFER_CHUNKS,
        });
        if let Err(err) = node.send(token, &target.device, &target.app, &chunk) {
            log::error!("[wellbeing-client] transfer {}: {err}", target.device_alias);
            return Sample {
                online: true,
                delay_ms,
                bytes_per_sec: 0,
                path,
            };
        }
    }
    let ack = wait_until(
        node,
        push,
        token,
        dir,
        xfers,
        proto::XFER_WAIT,
        |msg| matches!(msg, Message::XferAck { id, .. } if *id == xfer_id),
    );
    let bytes_per_sec = match ack {
        Some(Message::XferAck { received, .. }) => {
            proto::bytes_per_sec(received, xfer_started.elapsed())
        }
        _ => 0,
    };
    Sample {
        online: true,
        delay_ms,
        bytes_per_sec,
        path,
    }
}

fn wait_until(
    node: &NodeClient,
    push: &std::net::UdpSocket,
    token: &[u8; 16],
    dir: &Directory,
    xfers: &mut HashMap<([u8; 16], u64), Vec<u16>>,
    timeout: Duration,
    done: impl Fn(&Message) -> bool,
) -> Option<Message> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let datagram = match node::recv_push(push) {
            Ok(Some(datagram)) => datagram,
            Ok(None) => continue,
            Err(_) => continue,
        };
        let Some(message) = answer(node, token, dir, xfers, &datagram) else {
            continue;
        };
        if done(&message) {
            return Some(message);
        }
    }
    None
}

/// Replies to a ping or a finished transfer. Returns the decoded message.
fn answer(
    node: &NodeClient,
    token: &[u8; 16],
    dir: &Directory,
    xfers: &mut HashMap<([u8; 16], u64), Vec<u16>>,
    datagram: &[u8],
) -> Option<Message> {
    let (sender, payload) = node::decode_push(datagram)?;
    let message = proto::decode(payload).ok()?;
    let Some(device) = directory::device_of_app(dir, &sender) else {
        return Some(message);
    };
    match &message {
        Message::Ping { id } => {
            let pong = proto::encode(&Message::Pong { id: *id });
            let _ = node.send(token, &device, &sender, &pong);
        }
        Message::Xfer { id, index, total } if *total > 0 && *index < *total => {
            let seen = xfers.entry((sender, *id)).or_default();
            if !seen.contains(index) {
                seen.push(*index);
            }
            if index + 1 == *total || seen.len() as u16 == *total {
                let ack = proto::encode(&Message::XferAck {
                    id: *id,
                    received: seen.len() as u16,
                });
                let _ = node.send(token, &device, &sender, &ack);
            }
        }
        _ => {}
    }
    Some(message)
}

/// The node has no route from an app to another app on the same device.
/// Hand the report to the local server push port in the same shape the node uses.
fn deliver_local(sender_app: &[u8; 16], port: Option<u16>, payload: &[u8]) -> Result<(), String> {
    let port = port.ok_or_else(|| "local server has no push port".to_string())?;
    let mut datagram = Vec::with_capacity(17 + payload.len());
    datagram.push(0x04);
    datagram.extend_from_slice(sender_app);
    datagram.extend_from_slice(payload);
    let socket = std::net::UdpSocket::bind("127.0.0.1:0").map_err(|err| err.to_string())?;
    socket
        .send_to(&datagram, ("127.0.0.1", port))
        .map_err(|err| err.to_string())?;
    Ok(())
}

fn fetch(node: &NodeClient, token: &[u8; 16]) -> Result<Directory, String> {
    let reply = node.get_data(token).map_err(|err| err.to_string())?;
    directory::parse_get_data(&reply).map_err(|err| err.to_string())
}

fn next_id(seq: &mut u64) -> u64 {
    let id = unix_ms() ^ (*seq << 16);
    *seq = seq.wrapping_add(1);
    id
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}
