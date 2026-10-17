//! Minimal repro: SQL Server `datetimeoffset` decoded by tiberius into
//! `chrono::DateTime<Utc>` vs `chrono::DateTime<FixedOffset>` (and the `time`
//! crate's `OffsetDateTime` where the version supports it).
//!
//! The expected UTC instant is computed independently of tiberius' date/time
//! decoding: SQL Server renders `SWITCHOFFSET(v, '+00:00')` as ISO-8601 text
//! (style 127) and this program parses that text with chrono.
//!
//! Connection settings come from the environment:
//!   MSSQL_HOST (default 127.0.0.1), MSSQL_PORT (default 1433),
//!   MSSQL_USER (default sa), MSSQL_SA_PASSWORD (required),
//!   MSSQL_DB (default dto_repro).

use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime, Timelike, Utc};
use tiberius::{AuthMethod, Client, ColumnData, Config};
use tokio::net::TcpStream;
use tokio_util::compat::TokioAsyncWriteCompatExt;

const SCALES: [u8; 3] = [0, 3, 7];

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

/// ISO-8601 with 7 fractional digits (100 ns, SQL Server's finest scale).
fn fmt_naive(n: &NaiveDateTime) -> String {
    format!("{}.{:07}", n.format("%Y-%m-%dT%H:%M:%S"), n.nanosecond() / 100)
}

fn fmt_utc(dt: &DateTime<Utc>) -> String {
    format!("{}Z", fmt_naive(&dt.naive_utc()))
}

fn fmt_fixed(dt: &DateTime<FixedOffset>) -> String {
    format!("{}{}", fmt_naive(&dt.naive_local()), dt.format("%:z"))
}

/// (unix seconds, subsecond nanos): comparable across chrono and time without
/// the i64-nanosecond overflow that `timestamp_nanos` has outside 1677..2262.
fn instant_chrono<Tz: chrono::TimeZone>(dt: &DateTime<Tz>) -> (i64, u32) {
    (dt.timestamp(), dt.timestamp_subsec_nanos())
}

fn pass(ok: bool) -> &'static str {
    if ok {
        "PASS"
    } else {
        "FAIL"
    }
}

/// Raw TDS pieces of a DATETIMEOFFSET cell plus the wire date/time rendered
/// as-is (days since 0001-01-01 + 10^-scale s increments since midnight), with
/// no offset applied.
fn raw_pieces(cell: &ColumnData<'static>) -> (String, String) {
    match cell {
        ColumnData::DateTimeOffset(Some(dto)) => {
            let dt2 = dto.datetime2();
            let days = dt2.date().days();
            let incr = dt2.time().increments();
            let scale = dt2.time().scale();
            let date = NaiveDate::from_ymd_opt(1, 1, 1).unwrap()
                + chrono::Duration::days(days as i64);
            let ns = incr as i64 * 10i64.pow(9 - scale as u32);
            let time = NaiveTime::from_hms_opt(0, 0, 0).unwrap() + chrono::Duration::nanoseconds(ns);
            let wire = NaiveDateTime::new(date, time);
            (
                format!("days={} incr={} scale={} offset_min={}", days, incr, scale, dto.offset()),
                fmt_naive(&wire),
            )
        }
        ColumnData::DateTimeOffset(None) => ("NULL".into(), "NULL".into()),
        other => (format!("unexpected column type: {:?}", other), "-".into()),
    }
}

#[cfg(feature = "time-path")]
fn time_path(row: &tiberius::Row, idx: usize, expected: Option<(i64, u32)>, tzoffset: Option<i32>) -> (String, &'static str) {
    match row.try_get::<time::OffsetDateTime, _>(idx) {
        Ok(Some(odt)) => {
            let s = match odt.checked_to_offset(time::UtcOffset::UTC) {
                Some(u) => format!(
                    "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:07}Z (offset {}min)",
                    u.year(),
                    u8::from(u.month()),
                    u.day(),
                    u.hour(),
                    u.minute(),
                    u.second(),
                    u.nanosecond() / 100,
                    odt.offset().whole_minutes()
                ),
                // The instant lies outside the `time` crate's default +-9999 year range.
                None => format!(
                    "UTC instant beyond year 9999 (unix {}s, offset {}min)",
                    odt.unix_timestamp(),
                    odt.offset().whole_minutes()
                ),
            };
            let got = (odt.unix_timestamp(), odt.nanosecond());
            let ok = Some(got) == expected && Some(odt.offset().whole_minutes() as i32) == tzoffset;
            (s, pass(ok))
        }
        Ok(None) => ("NULL".into(), pass(expected.is_none())),
        Err(e) => (format!("error: {}", e), "FAIL"),
    }
}

#[cfg(not(feature = "time-path"))]
fn time_path(_: &tiberius::Row, _: usize, _: Option<(i64, u32)>, _: Option<i32>) -> (String, &'static str) {
    ("n/a (no `time` feature in this tiberius version)".into(), "n/a")
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let host = env_or("MSSQL_HOST", "127.0.0.1");
    let port: u16 = env_or("MSSQL_PORT", "1433").parse()?;
    let user = env_or("MSSQL_USER", "sa");
    let password = std::env::var("MSSQL_SA_PASSWORD").map_err(|_| "MSSQL_SA_PASSWORD is not set")?;
    let db = env_or("MSSQL_DB", "dto_repro");

    let mut config = Config::new();
    config.host(&host);
    config.port(port);
    config.database(&db);
    config.authentication(AuthMethod::sql_server(&user, &password));
    config.trust_cert(); // throwaway container with a self-signed certificate

    let tcp = TcpStream::connect(config.get_addr()).await?;
    tcp.set_nodelay(true)?;
    let mut client = Client::connect(config, tcp.compat_write()).await?;

    println!("tiberius version under test: {}", env!("CARGO_PKG_DESCRIPTION"));
    let ver = client
        .simple_query("SELECT CAST(SERVERPROPERTY('ProductVersion') AS varchar(40)) + ' ' + CAST(SERVERPROPERTY('Edition') AS varchar(60))")
        .await?
        .into_first_result()
        .await?;
    let ver: Option<&str> = ver[0].get(0);
    println!("server: {}", ver.unwrap_or("?"));
    println!();
    println!(
        "| id | scale | stored (CONVERT 126) | raw TDS pieces | wire date/time, no offset applied | expected UTC (SWITCHOFFSET text) | DateTime<Utc> | Utc - expected (min) | Utc | DateTime<FixedOffset> | FixedOffset | NaiveDateTime | time::OffsetDateTime as UTC | time |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|");

    let mut tally: std::collections::BTreeMap<&str, (u32, u32)> = Default::default();
    let mut bump = |path: &'static str, verdict: &str| {
        let e = tally.entry(path).or_insert((0, 0));
        match verdict {
            "PASS" => e.0 += 1,
            "FAIL" => e.1 += 1,
            _ => {}
        }
    };

    for scale in SCALES {
        // Columns: 0 id, 1 value, 2 as-stored text (style 126, keeps the offset),
        // 3 UTC text, 4 offset minutes, 5 style 127 text (SQL Server renders it in UTC).
        let sql = format!(
            "SELECT id, v{s}, CONVERT(varchar(40), v{s}, 126), \
             CONVERT(varchar(40), SWITCHOFFSET(v{s}, '+00:00'), 127), \
             DATEPART(TZOFFSET, v{s}), CONVERT(varchar(40), v{s}, 127) \
             FROM dbo.dto_repro ORDER BY id",
            s = scale
        );
        let rows = client.simple_query(sql).await?.into_first_result().await?;

        for row in rows {
            let id: i32 = row.get(0).unwrap();
            let stored: Option<String> = row.get::<&str, _>(2).map(str::to_owned);
            let utc_text: Option<String> = row.get::<&str, _>(3).map(str::to_owned);
            let tzoffset: Option<i32> = row.get(4);
            let style127: Option<String> = row.get::<&str, _>(5).map(str::to_owned);

            // Independent expectation: parse SQL Server's own UTC rendering.
            let expected: Option<DateTime<Utc>> =
                utc_text.as_deref().map(|t| DateTime::parse_from_rfc3339(t).expect("parse SWITCHOFFSET text").with_timezone(&Utc));
            // Sanity: the as-stored text with its offset must denote the same instant.
            if let (Some(s), Some(e)) = (stored.as_deref(), expected) {
                let local = DateTime::parse_from_rfc3339(s).expect("parse stored text");
                assert_eq!(instant_chrono(&local), instant_chrono(&e), "id {} scale {}: text renderings disagree", id, scale);
                assert_eq!(local.offset().local_minus_utc() / 60, tzoffset.unwrap(), "id {}: TZOFFSET mismatch", id);
                let z = DateTime::parse_from_rfc3339(style127.as_deref().unwrap()).expect("parse style 127 text");
                assert_eq!(instant_chrono(&z), instant_chrono(&e), "id {} scale {}: style 127 disagrees", id, scale);
            }
            let exp_instant = expected.as_ref().map(instant_chrono);

            // Path 1: chrono::DateTime<Utc>
            let (utc_s, utc_diff, utc_v) = match row.try_get::<DateTime<Utc>, _>(1) {
                Ok(Some(d)) => {
                    let diff = expected.map(|e| (d - e).num_minutes().to_string()).unwrap_or_default();
                    (fmt_utc(&d), diff, pass(Some(instant_chrono(&d)) == exp_instant))
                }
                Ok(None) => ("NULL".into(), String::new(), pass(expected.is_none())),
                Err(e) => (format!("error: {}", e), String::new(), "FAIL"),
            };

            // Path 2: chrono::DateTime<FixedOffset> (instant and offset must both match)
            let (fix_s, fix_v) = match row.try_get::<DateTime<FixedOffset>, _>(1) {
                Ok(Some(d)) => {
                    let ok = Some(instant_chrono(&d)) == exp_instant
                        && Some(d.offset().local_minus_utc() / 60) == tzoffset;
                    (fmt_fixed(&d), pass(ok))
                }
                Ok(None) => ("NULL".into(), pass(expected.is_none())),
                Err(e) => (format!("error: {}", e), "FAIL"),
            };

            // Path 3: NaiveDateTime (no mapping for datetimeoffset is expected)
            let naive_s = match row.try_get::<NaiveDateTime, _>(1) {
                Ok(Some(d)) => fmt_naive(&d),
                Ok(None) => "NULL".into(),
                Err(_) => "n/a (conversion error)".into(),
            };

            // Path 4: time::OffsetDateTime
            let (time_s, time_v) = time_path(&row, 1, exp_instant, tzoffset);

            bump("DateTime<Utc>", utc_v);
            bump("DateTime<FixedOffset>", fix_v);
            bump("time::OffsetDateTime", time_v);

            let cell = row.into_iter().nth(1).unwrap();
            let (raw, wire) = raw_pieces(&cell);

            println!(
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                id,
                scale,
                stored.as_deref().unwrap_or("NULL"),
                raw,
                wire,
                expected.as_ref().map(fmt_utc).unwrap_or_else(|| "NULL".into()),
                utc_s,
                utc_diff,
                utc_v,
                fix_s,
                fix_v,
                naive_s,
                time_s,
                time_v
            );
        }
    }

    println!();
    for (path, (p, f)) in &tally {
        if p + f > 0 {
            println!("summary {}: {} PASS, {} FAIL", path, p, f);
        } else {
            println!("summary {}: n/a", path);
        }
    }
    Ok(())
}
