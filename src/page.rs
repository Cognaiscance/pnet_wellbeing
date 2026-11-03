//! The status page served on loopback and mounted at `/apps/wellbeing/`.

use crate::store::Report;

pub fn render(rows: &[Report]) -> String {
    let mut body = String::from(
        "<!DOCTYPE html>\n<html><head><meta charset=\"utf-8\"><title>Wellbeing</title>\n\
         <style>\
         body{font-family:sans-serif;max-width:56rem;margin:2rem auto;padding:0 1rem;color:#1c2430}\
         h1{font-size:1.4rem}\
         table{border-collapse:collapse;width:100%}\
         th,td{text-align:left;padding:.4rem .6rem;border-bottom:1px solid #d5dbe3}\
         .off{color:#8a3b3b}\
         .on{color:#1d6b3a}\
         </style></head><body>\n\
         <h1>Wellbeing</h1>\n\
         <p>Latest check from each client to the other devices it can name. \
         Delay is the round trip. Rate is the sample that came back.</p>\n",
    );
    if rows.is_empty() {
        body.push_str("<p>No checks stored yet.</p>\n");
    } else {
        body.push_str(
            "<table><thead><tr><th>From</th><th>To</th><th>Whose</th><th>Status</th>\
             <th>Delay</th><th>Rate</th><th>When</th></tr></thead><tbody>\n",
        );
        for row in rows {
            let status = if row.online {
                "<span class=\"on\">online</span>"
            } else {
                "<span class=\"off\">not online</span>"
            };
            body.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{} ms</td><td>{}</td><td>{}</td></tr>\n",
                esc(&label(&row.from_alias, &row.from_id)),
                esc(&label(&row.to_alias, &row.to_id)),
                esc(&row.who),
                status,
                row.delay_ms,
                rate_label(row.bytes_per_sec),
                esc(&format_time(row.at_ms)),
            ));
        }
        body.push_str("</tbody></table>\n");
    }
    body.push_str("<p><a href=\"/\">Portal home</a></p>\n</body></html>\n");
    body
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
        }];
        let html = render(&rows);
        assert!(html.contains("laptop"));
        assert!(html.contains("online"));
        assert!(html.contains("18 ms"));
        assert!(html.contains("chad"));
        assert!(render(&[]).contains("No checks stored yet"));
        assert_eq!(format_time(0), "1970-01-01 00:00:00 UTC");
    }
}
