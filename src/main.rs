#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate slog;
extern crate serde_json;

mod domain;
mod parser;

use self::parser::ReportParser;
use fnv::FnvHashSet;
use reqwest::{header, Client, StatusCode};
use std::io::Read;
use std::thread;
use std::time::Duration;
use wx::domain::{FetchFailure, WxApp};
use wx::error::{Error, WxError};
use wx::util::Logger;

#[derive(Debug)]
pub struct Comparison {
    latest_set: FnvHashSet<String>,
    new: Vec<String>,
}

#[derive(Serialize)]
pub struct Config {
    pub app_name: &'static str,
    pub api_url: &'static str,
    pub poll_interval_ms: u64,
    pub user_agent: &'static str,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            app_name: "sn_loader",
            api_url: "http://www.spotternetwork.org/feeds/reports.txt",
            poll_interval_ms: 60_000,
            user_agent: "sigtor.org",
        }
    }
}

fn main() {
    let config = Config::default();
    let logger = Logger::new(&config.app_name);
    let http_client = Client::new();
    let store_client = wx::store::Client::new();
    let parser = ReportParser::new();
    let mut seen: FnvHashSet<String> = FnvHashSet::default();

    info!(logger, "initializing"; "config" => serde_json::to_string(&config).unwrap());
    // TODO test loading non-utf8 file and figure out where it breaks in this module

    loop {
        let response = fetch_reports(&http_client, &config.api_url, &config.user_agent);

        match response {
            Ok(body) => {
                let comparison = get_comparison(&body, seen);
                seen = comparison.latest_set;

                comparison
                    .new
                    .iter()
                    .map(|report| parser.parse(report))
                    .for_each(|event| match event {
                        Ok(event) => {
                            if event.is_some() {
                                match store_client.put_event(&event.unwrap()) {
                                    Ok(_) => info!(logger, "stored event";),
                                    Err(e) => {
                                        let reason = format!("unable to store event: {}", e);
                                        error!(logger, "processing"; "reason" => reason);
                                    }
                                }
                            }
                        }
                        Err(e) => warn!(logger, "parse"; "reason" => e.to_string()),
                    });
            }
            Err(e) => {
                warn!(logger, "fetch_reports"; "error" => e.to_string());
                // let failure = FetchFailure {
                //     app: WxApp::SpotterNetworkLoader,
                //     ingest_ts: 0,
                // };
                // store_client.put_fetch_failure(&failure).unwrap();
            }
        }

        thread::sleep(Duration::from_millis(config.poll_interval_ms));
    }
}

fn get_comparison(body: &str, seen: FnvHashSet<String>) -> Comparison {
    let latest_set: FnvHashSet<String> = body
        .lines()
        .filter(|x| x.starts_with("Icon:"))
        .map(|x| normalize_line(x).to_string())
        .collect();

    let new: Vec<String> = latest_set
        .iter()
        .filter_map(|x| {
            if !seen.contains(x) {
                Some(x.to_string())
            } else {
                None
            }
        })
        .collect();

    Comparison { latest_set, new }
}

fn fetch_reports(client: &Client, url: &str, user_agent: &str) -> Result<String, Error> {
    let mut response = client
        .get(url)
        .header(header::USER_AGENT, user_agent)
        .send()?;

    match response.status() {
        StatusCode::OK => {} // don't exit early
        _ => {
            let reason = format!("Unexpected status code: {}", response.status());
            return Err(Error::Wx(<WxError>::new(&reason)));
        }
    }

    let mut body = String::new();
    response.read_to_string(&mut body)?;

    Ok(body)
}

/**
 * Normalizes raw report lines as returned by the SpotterNetwork API. Since there is no offset,
 * you will see the same report multiple times and need to de-dupe. Unfortunately, the same
 * report will have the icon image digit change as the report ages so we need to normalize.
 */
fn normalize_line(line: &str) -> String {
    line.replace(",000,3", ",000,0")
        .replace(",000,4", ",000,0")
        .replace(",000,5", ",000,0")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    #[test]
    fn normalize_line_should_zero_icon_digit() {
        let line = r#"Icon: 47.617706,-111.215248,000,4,4,"Reported By: Test User\nHail\nTime: 2018-09-20 22:49:29 UTC\nSize: 0.75" (Penny)\nNotes: None""#;
        let expected = r#"Icon: 47.617706,-111.215248,000,0,4,"Reported By: Test User\nHail\nTime: 2018-09-20 22:49:29 UTC\nSize: 0.75" (Penny)\nNotes: None""#;
        let normalized = normalize_line(line);
        assert_eq!(normalized, expected);
    }

    #[test]
    fn empty_report_should_return_no_seen_or_unseen() {
        let mut file = File::open("data/reports-empty").expect("unable to open file");
        let mut body = String::new();
        file.read_to_string(&mut body).expect("unable to read file");
        let comparison = get_comparison(&body, FnvHashSet::default());
        assert_eq!(comparison.latest_set.len(), 0);
        assert_eq!(comparison.new.len(), 0);
    }

    #[test]
    fn no_current_seen_should_return_all_reports() {
        let mut file = File::open("data/reports").expect("unable to open file");
        let mut body = String::new();
        file.read_to_string(&mut body).expect("unable to read file");
        let comparison = get_comparison(&body, FnvHashSet::default());
        assert_eq!(comparison.latest_set.len(), 23);
        assert_eq!(comparison.new.len(), 23);
    }

    #[test]
    fn same_report_different_age_digit_should_be_deduped() {
        let body = r#"Icon: 47.617706,-111.215248,000,4,4,"Reported By: Test User\nHail\nTime: 2018-09-20 22:39:00 UTC\nSize: 0.75" (Penny)\nNotes: None""#;
        let comparison = get_comparison(&body, FnvHashSet::default());
        assert_eq!(comparison.latest_set.len(), 1);
        assert_eq!(comparison.new.len(), 1);

        let body = r#"Icon: 47.617706,-111.215248,000,5,4,"Reported By: Test User\nHail\nTime: 2018-09-20 22:39:00 UTC\nSize: 0.75" (Penny)\nNotes: None"
            Icon: 47.617706,-111.215248,000,6,4,"Reported By: Test User\nHail\nTime: 2018-09-20 22:39:00 UTC\nSize: 0.75" (Penny)\nNotes: None""#;
        let comparison = get_comparison(&body, comparison.latest_set);
        assert_eq!(comparison.latest_set.len(), 1);
        assert_eq!(comparison.new.len(), 0);
    }

    #[test]
    fn get_comparison_should_handle_previously_seen_reports() {
        let mut file = File::open("data/reports").expect("unable to open file");
        let mut body = String::new();
        file.read_to_string(&mut body).expect("unable to read file");

        let seen: FnvHashSet<String> = vec![
            "Icon: 41.338901,-96.059708,000,0,5,\"Reported By: Will Dupe\\nHigh Wind\\nTime: 2018-09-21 00:26:06 UTC\\n50 mphNotes: None\"".to_string(),
            "Icon: 47.617706,-111.215248,000,0,4,\"Reported By: Will Dupe\\nHail\\nTime: 2018-09-20 22:49:29 UTC\\nSize: 0.75\" (Penny)\\nNotes: None\"".to_string(),
            "Icon: 43.112000,-94.610001,000,0,6,\"Reported By: Will Dupe\\nFlooding\\nTime: 2018-09-20 22:58:00 UTC\\nNotes: Water over road on US 18\"".to_string(),
            "Icon: 41.338715,-96.059563,000,0,5,\"Reported By: Will Dupe\\nHigh Wind\\nTime: 2018-09-21 00:34:00 UTC\\n60 mphNotes: Wind gusting to 63mph\"".to_string(),
            "Icon: 35.851399,-90.708198,000,0,8,\"Reported By: Will Dupe\\nOther - See Note\\nTime: 2018-11-14 20:22:00 UTC\\nNotes: i got snow and a little of sleet\"".to_string(),
            "Icon: 41.230400,-95.850403,000,0,3,\"Reported By: Will Dupe\\nNot Rotating Wall Cloud\\nTime: 2018-09-21 00:34:00 UTC\\nNotes: None\"".to_string(),
        ].into_iter().collect();

        let seen_length = seen.len();
        let comparison = get_comparison(&body, seen);

        assert_eq!(comparison.latest_set.len(), 23);
        assert_eq!(
            comparison.new.len(),
            comparison.latest_set.len() - seen_length
        );
    }
}
