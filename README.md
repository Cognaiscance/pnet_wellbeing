# pnet_wellbeing

Checks whether this device can reach the other devices that run the wellbeing client, and shows the latest results on the owner portal.

pNet only carries the bytes. The two programs register as different apps and share the protocol `application/wellbeing`.

| Program | Where it runs | What it does |
|---------|----------------|--------------|
| `wellbeing-client` | Every device, server-grade and device-grade | Probes each other device of this user, and each friend's device, that has an approved client. Sends one report to the server. |
| `wellbeing-server` | Server-grade nodes | The top-ranked server stores the reports and mounts `/apps/wellbeing/`. Any other copy stands by and does not open the store. |

Top-ranked means the server-grade device with the lowest `sg_rank`. Rank 0 sorts last. The same rank uses the lower device id. A device-grade node never holds the store.

Each check records three things:

- status: `online` when the other client answers, otherwise `not online`
- delay: the ping's round trip, in milliseconds
- path: `server`, `lan`, `punch`, or `session`, from the node's send reply. `unknown` when the node did not say
- rate: bytes per second from an eight-chunk sample (8 × 512 bytes) that the other client acknowledges

The client repeats the checks every 60 seconds (`WELLBEING_INTERVAL_SECS`, minimum 5). A client that is still waiting for approval does not send reports. Reports are accepted only from an approved `wellbeing-client` on this user's own devices. A friend's client reports to that friend's server.

## Run

On each device:

```bash
cargo run --bin wellbeing-client
```

On the server-grade nodes:

```bash
cargo run --bin wellbeing-server
```

Approve both apps on the portal. Sign in and open **Wellbeing** on the top-ranked server (`/apps/wellbeing/`).

## Environment

| Variable | Default | Meaning |
|----------|---------|---------|
| `PNET_ADDR` | `127.0.0.1:7777` | Local pNet node |
| `PNET_PORTAL` | `http://127.0.0.1:8777` | Portal, used by the server to register the page |
| `WELLBEING_PUSH_PORT` | `8800` client, `8801` server | Loopback port where this process receives app pushes |
| `WELLBEING_HTTP_PORT` | `8810` | Loopback page on the active server |
| `WELLBEING_INTERVAL_SECS` | `60` | How often a client runs the checks |
| `WELLBEING_DIR` | `~/.pnet/wellbeing` | Report file `results` |
| `WELLBEING_TOKEN_FILE` | `~/.pnet/wellbeing-client/token` or `wellbeing-server` | Saved app token. Leave unset when both programs run on one machine. |

Ctrl+C stops the process. When the active server exits, the portal mount stays until pNet restarts or another server registers the same slug. A server that loses the top rank removes its mount.

## Check

```bash
cargo test
```
