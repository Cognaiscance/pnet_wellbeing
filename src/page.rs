//! The status page served on loopback and mounted at `/apps/wellbeing/`.

use crate::store::Report;

pub fn render(rows: &[Report]) -> String {
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
         <button class=\"theme-toggle\" type=\"button\">Light</button>\n\
         <main>\n<h1>Wellbeing</h1>\n\
         <p class=\"lead\">Each wellbeing client, and the connections it checked. \
         Delay is the round trip. Rate is the sample that came back.</p>\n",
    );
    if rows.is_empty() {
        body.push_str("<p class=\"empty\">No checks stored yet.</p>\n");
    } else {
        let mut seen: Vec<[u8; 16]> = Vec::new();
        for row in rows {
            if !seen.contains(&row.from_id) {
                seen.push(row.from_id);
            }
        }
        for from_id in seen {
            let first = rows.iter().find(|row| row.from_id == from_id).unwrap();
            body.push_str("<section class=\"card\"><h2>wellbeing-client on ");
            body.push_str(&esc(&label(&first.from_alias, &from_id)));
            body.push_str(
                "</h2><table><thead><tr><th>To</th><th>Whose</th><th>Status</th>\
                 <th>Delay</th><th>Rate</th><th>When</th></tr></thead><tbody>\n",
            );
            for row in rows.iter().filter(|row| row.from_id == from_id) {
                let status = if row.online {
                    "<span class=\"on\">online</span>"
                } else {
                    "<span class=\"off\">not online</span>"
                };
                body.push_str(&format!(
                    "<tr><td>{}</td><td>{}</td><td>{}</td><td>{} ms</td><td>{}</td><td>{}</td></tr>\n",
                    esc(&label(&row.to_alias, &row.to_id)),
                    esc(&row.who),
                    status,
                    row.delay_ms,
                    rate_label(row.bytes_per_sec),
                    esc(&format_time(row.at_ms)),
                ));
            }
            body.push_str("</tbody></table></section>\n");
        }
    }
    body.push_str(
        "<p><a href=\"/\">Portal home</a></p>\n</main>\n<script>\
         (function () {\
           var key = \"pnet-theme\";\
           function current() {\
             return document.documentElement.getAttribute(\"data-theme\") === \"light\" ? \"light\" : \"dark\";\
           }\
           function paint(theme) {\
             document.documentElement.setAttribute(\"data-theme\", theme);\
             var meta = document.querySelector('meta[name=\"theme-color\"]');\
             if (meta) meta.setAttribute(\"content\", theme === \"light\" ? \"#f3eee4\" : \"#070605\");\
             var next = theme === \"dark\" ? \"light\" : \"dark\";\
             document.querySelectorAll(\".theme-toggle\").forEach(function (btn) {\
               btn.textContent = theme === \"dark\" ? \"Light\" : \"Dark\";\
               btn.setAttribute(\"aria-label\", \"Switch to \" + next + \" mode\");\
             });\
           }\
           paint(current());\
           document.addEventListener(\"click\", function (ev) {\
             var btn = ev.target.closest && ev.target.closest(\".theme-toggle\");\
             if (!btn) return;\
             var next = current() === \"dark\" ? \"light\" : \"dark\";\
             try { localStorage.setItem(key, next); } catch (e) {}\
             paint(next);\
           });\
           window.addEventListener(\"storage\", function (ev) {\
             if (ev.key === key && (ev.newValue === \"light\" || ev.newValue === \"dark\")) paint(ev.newValue);\
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
  --ok: #8fbf73; --bad: #e07a6a; --link: #e4c97a;
}
html[data-theme="light"] {
  color-scheme: light;
  --bg: #f3eee4; --bg-glow: #f0e0be; --bg-elev: #fffdf8;
  --ink: #241c14; --muted: #6b5844; --gold: #8d6818; --gold-hover: #6f5010;
  --gold-ink: #fffdf8; --line: rgba(36, 28, 20, 0.1);
  --card-line: rgba(141, 104, 24, 0.28); --line-strong: rgba(141, 104, 24, 0.38);
  --shadow: 0 12px 32px rgba(70, 48, 16, 0.08);
  --ok: #1b6b34; --bad: #a3261c; --link: #8a6412;
}
* { box-sizing: border-box; }
html, body { margin: 0; min-height: 100%; }
body {
  background: radial-gradient(1100px 520px at 50% -12%, var(--bg-glow) 0%, transparent 58%), var(--bg);
  color: var(--ink);
  font-family: "Segoe UI", system-ui, -apple-system, sans-serif;
  line-height: 1.5;
}
h1, h2 { font-family: Georgia, "Iowan Old Style", Palatino, serif; font-weight: 600; }
h1 { margin: 0 0 0.4rem; font-size: 1.9rem; }
h2 { margin: 0 0 0.8rem; font-size: 1.15rem; }
a { color: var(--link); }
a:hover { color: var(--gold-hover); }
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
button.theme-toggle {
  position: fixed; top: 1rem; right: 1rem; z-index: 2;
  background: transparent; color: var(--ink); border: 1px solid var(--line-strong);
  border-radius: 8px; font-weight: 600; padding: .28rem .7rem; cursor: pointer;
}
button.theme-toggle:hover { color: var(--gold); border-color: var(--gold); }
"#;

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
        }];
        let html = render(&rows);
        assert!(html.contains("wellbeing-client on laptop"));
        assert!(html.contains("online"));
        assert!(html.contains("18 ms"));
        assert!(html.contains("chad"));
        assert!(html.contains("pnet-theme"));
        assert!(html.contains("data-theme=\"dark\""));
        assert!(render(&[]).contains("No checks stored yet"));
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
            },
        ];
        let html = render(&rows);
        let laptop = html.find("wellbeing-client on laptop").unwrap();
        let pi = html.find("wellbeing-client on pi").unwrap();
        assert!(laptop < pi);
        assert_eq!(html.matches("wellbeing-client on").count(), 2);
    }
}
