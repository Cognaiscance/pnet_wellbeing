//! The status page served on loopback and mounted at `/apps/wellbeing/`.

use crate::store::Report;

pub fn render(rows: &[Report], owner_alias: &str) -> String {
    let mut body = String::from(
        "<!DOCTYPE html>\n<html lang=\"en\" data-theme=\"dark\"><head><meta charset=\"utf-8\">\
         <meta name=\"theme-color\" content=\"#070605\"><title>Wellbeing</title>\n\
         <script>\
         (function () {\
           var theme = \"dark\";\
           try {\
             var saved = localStorage.getItem(\"pnet-theme\");\
             if (saved === \"light\" || saved === \"dark\") theme = saved;\
           } catch (e) {}\
           document.documentElement.setAttribute(\"data-theme\", theme);\
           var meta = document.querySelector('meta[name=\"theme-color\"]');\
           if (meta) meta.setAttribute(\"content\", theme === \"light\" ? \"#f3eee4\" : \"#070605\");\
         })();\
         </script>\n<style>",
    );
    body.push_str(PAGE_CSS);
    body.push_str(
        "</style></head><body>\n\
         <main>\n<h1>Wellbeing</h1>\n\
         <p class=\"lead\">A quick look at the status as seen from each device.</p>\n\
         Delay is the round trip. Rate is the sample that came back. \
         Times use this browser’s time zone.</p>\n",
    );
    if rows.is_empty() {
        body.push_str("<p class=\"empty\">No checks stored yet.</p>\n");
    } else {
        for who in people(rows) {
            body.push_str("<section class=\"person\"><h2>");
            body.push_str(&esc(&person_label(who, owner_alias)));
            body.push_str("</h2>\n");
            let mut seen: Vec<[u8; 16]> = Vec::new();
            for row in rows.iter().filter(|row| row.who == who) {
                if !seen.contains(&row.from_id) {
                    seen.push(row.from_id);
                }
            }
            for from_id in seen {
                let first = rows
                    .iter()
                    .find(|row| row.who == who && row.from_id == from_id)
                    .unwrap();
                body.push_str("<section class=\"card\"><h3>");
                body.push_str(&esc(&label(&first.from_alias, &from_id)));
                body.push_str(
                    "</h3><table><thead><tr><th>To</th><th>Status</th>\
                     <th>Delay</th><th>Path</th><th>Rate</th><th>When</th></tr></thead><tbody>\n",
                );
                for row in rows
                    .iter()
                    .filter(|row| row.who == who && row.from_id == from_id)
                {
                    let status = if row.online {
                        "<span class=\"on\">online</span>"
                    } else {
                        "<span class=\"off\">not online</span>"
                    };
                    body.push_str(&format!(
                        "<tr><td>{}</td><td>{}</td><td>{} ms</td><td>{}</td><td>{}</td><td><time data-ms=\"{}\">{}</time></td></tr>\n",
                        esc(&label(&row.to_alias, &row.to_id)),
                        status,
                        row.delay_ms,
                        esc(&row.path),
                        rate_label(row.bytes_per_sec),
                        row.at_ms,
                        esc(&format_time(row.at_ms)),
                    ));
                }
                body.push_str("</tbody></table></section>\n");
            }
            body.push_str("</section>\n");
        }
    }
    body.push_str(
        "</main>\n<script>\
         (function () {\
           var key = \"pnet-theme\";\
           function paint(theme) {\
             document.documentElement.setAttribute(\"data-theme\", theme);\
             var meta = document.querySelector('meta[name=\"theme-color\"]');\
             if (meta) meta.setAttribute(\"content\", theme === \"light\" ? \"#f3eee4\" : \"#070605\");\
           }\
           window.addEventListener(\"storage\", function (ev) {\
             if (ev.key === key && (ev.newValue === \"light\" || ev.newValue === \"dark\")) paint(ev.newValue);\
           });\
         })();\
         (function () {\
           var fmt = new Intl.DateTimeFormat(undefined, {\
             year: \"numeric\", month: \"2-digit\", day: \"2-digit\",\
             hour: \"2-digit\", minute: \"2-digit\", second: \"2-digit\",\
             hourCycle: \"h23\", timeZoneName: \"short\"\
           });\
           document.querySelectorAll(\"time[data-ms]\").forEach(function (el) {\
             var ms = Number(el.getAttribute(\"data-ms\"));\
             if (!Number.isFinite(ms)) return;\
             el.textContent = fmt.format(new Date(ms));\
           });\
         })();\
         </script>\n</body></html>\n",
    );
    body
}

const PAGE_CSS: &str = r#"
:root, html[data-theme="dark"] {
  color-scheme: dark;
  --bg: #070605; --bg-glow: #2a2218; --bg-elev: #14110d;
  --ink: #f4efe6; --muted: #b7aa96; --gold: #d4b15a; --gold-hover: #e4c56e;
  --gold-ink: #1a1408; --line: rgba(244, 239, 230, 0.12);
  --card-line: rgba(212, 177, 90, 0.32); --line-strong: rgba(212, 177, 90, 0.45);
  --shadow: 0 16px 36px rgba(0, 0, 0, 0.45);
  --ok: #8fbf73; --bad: #e07a6a;
}
html[data-theme="light"] {
  color-scheme: light;
  --bg: #f3eee4; --bg-glow: #f0e0be; --bg-elev: #fffdf8;
  --ink: #241c14; --muted: #6b5844; --gold: #8d6818; --gold-hover: #6f5010;
  --gold-ink: #fffdf8; --line: rgba(36, 28, 20, 0.1);
  --card-line: rgba(141, 104, 24, 0.28); --line-strong: rgba(141, 104, 24, 0.38);
  --shadow: 0 12px 32px rgba(70, 48, 16, 0.08);
  --ok: #1b6b34; --bad: #a3261c;
}
* { box-sizing: border-box; }
html, body { margin: 0; min-height: 100%; }
body {
  background: radial-gradient(1100px 520px at 50% -12%, var(--bg-glow) 0%, transparent 58%), var(--bg);
  color: var(--ink);
  font-family: "Segoe UI", system-ui, -apple-system, sans-serif;
  line-height: 1.5;
}
h1, h2, h3 { font-family: Georgia, "Iowan Old Style", Palatino, serif; font-weight: 600; }
h1 { margin: 0 0 0.4rem; font-size: 1.9rem; }
.person { margin: 1.6rem 0 0.2rem; }
.person > h2 { margin: 0; font-size: 1.45rem; }
h3 { margin: 0 0 0.8rem; font-size: 1.15rem; }
main { max-width: 56rem; margin: 0 auto; padding: 2rem 1.2rem 3rem; }
.lead, .empty { color: var(--muted); }
.card {
  background: var(--bg-elev); border: 1px solid var(--card-line); border-radius: 14px;
  padding: 1.2rem 1.4rem; box-shadow: var(--shadow); margin: 1rem 0;
}
table { border-collapse: collapse; width: 100%; }
th, td { text-align: left; padding: .45rem .6rem; border-bottom: 1px solid var(--line); }
th { color: var(--muted); font-weight: 600; font-size: .85rem; }
.on { color: var(--ok); font-weight: 600; }
.off { color: var(--bad); font-weight: 600; }
"#;

/// `you` first, then other contacts in the order they appear.
fn people(rows: &[Report]) -> Vec<&str> {
    let mut out: Vec<&str> = Vec::new();
    if rows.iter().any(|row| row.who == "you") {
        out.push("you");
    }
    for row in rows {
        if row.who != "you" && !out.contains(&row.who.as_str()) {
            out.push(&row.who);
        }
    }
    out
}

fn person_label(who: &str, owner_alias: &str) -> String {
    if who == "you" {
        if owner_alias.is_empty() {
            "you".to_string()
        } else {
            owner_alias.to_string()
        }
    } else if who.is_empty() {
        "Unknown".to_string()
    } else {
        who.to_string()
    }
}

fn label(alias: &str, id: &[u8; 16]) -> String {
    if alias.is_empty() {
        crate::store::hex(&id[..4])
    } else {
        alias.to_string()
    }
}

fn rate_label(bytes_per_sec: u32) -> String {
    if bytes_per_sec == 0 {
        return "0 B/s".to_string();
    }
    if bytes_per_sec >= 1_000_000 {
        format!("{:.1} MB/s", bytes_per_sec as f64 / 1_000_000.0)
    } else if bytes_per_sec >= 1_000 {
        format!("{:.1} KB/s", bytes_per_sec as f64 / 1_000.0)
    } else {
        format!("{bytes_per_sec} B/s")
    }
}

fn esc(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// UTC `YYYY-MM-DD HH:MM:SS` from Unix milliseconds.
pub fn format_time(at_ms: u64) -> String {
    let secs = at_ms / 1000;
    let days = secs / 86_400;
    let tod = secs % 86_400;
    let hour = tod / 3600;
    let min = (tod % 3600) / 60;
    let sec = tod % 60;
    let (year, month, day) = civil_from_days(days as i64);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{min:02}:{sec:02} UTC")
}

fn civil_from_days(days: i64) -> (i32, u32, u32) {
    // Howard Hinnant, days since 1970-01-01.
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u32, d as u32)
}

pub fn http_response(body: &str) -> Vec<u8> {
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let mut out = header.into_bytes();
    out.extend_from_slice(body.as_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_shows_online_and_offline() {
        let rows = vec![Report {
            from_id: [1; 16],
            from_alias: "laptop".into(),
            to_id: [2; 16],
            to_alias: "x1".into(),
            who: "chad".into(),
            online: true,
            delay_ms: 18,
            bytes_per_sec: 4096,
            at_ms: 1_700_000_000_000,
            path: "lan".into(),
        }];
        let html = render(&rows, "william");
        assert!(html.contains("<h3>laptop</h3>"));
        assert!(!html.contains("wellbeing-client on"));
        assert!(!html.contains("<th>Whose</th>"));
        assert!(html.contains("<h2>chad</h2>"));
        assert!(html.contains("online"));
        assert!(html.contains("18 ms"));
        assert!(html.contains(">lan<"));
        assert!(html.contains("data-ms=\"1700000000000\""));
        assert!(html.contains("time[data-ms]"));
        assert!(html.contains("timeZoneName"));
        assert!(html.contains("pnet-theme"));
        assert!(html.contains("data-theme=\"dark\""));
        assert!(!html.contains("theme-toggle"));
        assert!(!html.contains("Portal home"));
        assert!(render(&[], "william").contains("No checks stored yet"));
        assert_eq!(format_time(0), "1970-01-01 00:00:00 UTC");
    }

    #[test]
    fn page_groups_connections_by_client() {
        let rows = vec![
            Report {
                from_id: [1; 16],
                from_alias: "laptop".into(),
                to_id: [2; 16],
                to_alias: "n64".into(),
                who: "you".into(),
                online: true,
                delay_ms: 7,
                bytes_per_sec: 1000,
                at_ms: 1_700_000_000_000,
                path: "server".into(),
            },
            Report {
                from_id: [3; 16],
                from_alias: "pi".into(),
                to_id: [1; 16],
                to_alias: "laptop".into(),
                who: "you".into(),
                online: false,
                delay_ms: 0,
                bytes_per_sec: 0,
                at_ms: 1_700_000_000_000,
                path: "unknown".into(),
            },
        ];
        let html = render(&rows, "william");
        let laptop = html.find("<h3>laptop</h3>").unwrap();
        let pi = html.find("<h3>pi</h3>").unwrap();
        assert!(html.find("<h2>william</h2>").unwrap() < laptop);
        assert!(laptop < pi);
        assert_eq!(html.matches("<h2>william</h2>").count(), 1);
    }

    #[test]
    fn page_puts_you_before_other_people() {
        let rows = vec![
            Report {
                from_id: [3; 16],
                from_alias: "n64".into(),
                to_id: [4; 16],
                to_alias: "x1".into(),
                who: "chad".into(),
                online: true,
                delay_ms: 40,
                bytes_per_sec: 800,
                at_ms: 1_700_000_000_000,
                path: "punch".into(),
            },
            Report {
                from_id: [1; 16],
                from_alias: "n64".into(),
                to_id: [2; 16],
                to_alias: "sanosuke".into(),
                who: "you".into(),
                online: true,
                delay_ms: 7,
                bytes_per_sec: 1000,
                at_ms: 1_700_000_000_000,
                path: "server".into(),
            },
        ];
        let html = render(&rows, "william");
        let you = html.find("<h2>william</h2>").unwrap();
        let chad = html.find("<h2>chad</h2>").unwrap();
        assert!(you < chad);
        assert!(html[you..chad].contains("<h3>n64</h3>"));
        assert!(html[chad..].contains("<h3>n64</h3>"));
        assert!(!html.contains("Whose"));
    }
}
