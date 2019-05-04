use crate::domain::Hazard;
use chrono::prelude::*;
use regex::Regex;
use wx::domain::{Coordinates, Event, EventType, Location, Report, Units};
use wx::error::{Error, WxError};

const REPORT_PATTERN: &str = r"Icon: (?P<lat>\d{2}\.\d{6}),(?P<lon>-\d{2,3}\.\d{6}),000,\d,(?P<hazard_code>\d),.Reported By: (?P<reporter>.+)\\n.+\\nTime: (?P<ts>.+) UTC(?:\\nSize: (?P<size>\d{1,2}\.\d{2}).+?)*(?:\\n(?P<mph>\d{1,3}) mph)*(?P<measured> \[Measured\])*.+otes: (?P<notes>.+).$";

pub struct ReportParser {
    pub compiled_regex: Regex,
}

impl ReportParser {
    pub fn new() -> ReportParser {
        let compiled_regex = regex::Regex::new(REPORT_PATTERN).unwrap();

        ReportParser { compiled_regex }
    }

    pub fn parse(&self, report: &str) -> Result<Option<Event>, Error> {
        let captures = self.compiled_regex.captures(report);

        if captures.is_none() {
            let reason = "invalid spotter network report format";
            return Err(Error::Wx(<WxError>::new(&reason)));
        }

        let captures = captures.unwrap();

        let hazard = Hazard::get_by_code(captures.name("hazard_code").unwrap().as_str())?;
        let notes = captures.name("notes").unwrap().as_str();
        let reporter = captures.name("reporter").unwrap().as_str();

        // Skip Other/None reports since they're essentially worthless
        if hazard == Hazard::Other && notes == "None" {
            return Ok(None);
        }

        let mut report = Report {
            hazard: hazard.to_hazard_type(),
            magnitude: None,
            report_ts: None, // not set for SN reports
            reporter: reporter.to_string(),
            units: None,
            was_measured: None,
        };

        if captures.name("measured").is_some() {
            report.was_measured = Some(true);
        }

        let mph_cap = captures.name("mph");
        let size_cap = captures.name("size");

        if mph_cap.is_some() {
            report.magnitude = Some(mph_cap.unwrap().as_str().parse()?);
            report.units = Some(Units::Mph);
        } else if size_cap.is_some() {
            report.magnitude = Some(size_cap.unwrap().as_str().parse()?);
            report.units = Some(Units::Inches);
        }

        let location = Some(Location {
            county: None,
            wfo: None,
            point: Some(Coordinates {
                lat: captures.name("lat").unwrap().as_str().parse()?,
                lon: captures.name("lon").unwrap().as_str().parse()?,
            }),
            poly: None,
        });

        let event_ts = Utc
            .datetime_from_str(captures.name("ts").unwrap().as_str(), "%Y-%m-%d %H:%M:%S")?
            .timestamp() as u64
            * 1_000_000;

        let text = if notes == "None" {
            format!("{} reported by {}", hazard.to_string(), reporter)
        } else {
            format!("{} reported by {}. {}", hazard.to_string(), reporter, notes)
        };
        let title = format!("Report: {}", hazard.to_string());

        let event = Event {
            event_ts,
            event_type: EventType::SnReport,
            expires_ts: None,
            fetch_status: None,
            image_uri: None,
            ingest_ts: 0, // set when storing
            location,
            md: None,
            outlook: None,
            report: Some(report),
            text: Some(text),
            title,
            valid_ts: None,
            warning: None,
            watch: None,
        };

        Ok(Some(event))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::{BufRead, BufReader};
    use wx::domain::HazardType;

    #[test]
    fn parse_should_skip_empty_other_reports() {
        let parser = ReportParser::new();
        let reports_file = File::open("data/reports-other-none").unwrap();
        let reader = BufReader::new(reports_file);

        reader
            .lines()
            .map(|x| x.unwrap())
            .filter(|x| x.starts_with("Icon:"))
            .for_each(|x| {
                let message = parser.parse(&x);
                assert!(message.unwrap().is_none());
            });
    }

    #[test]
    fn parse_should_return_an_event_with_all_required_fields() {
        let parser = ReportParser::new();
        let report = r#"Icon: 43.112000,-94.639999,000,3,5,"Reported By: Test Human\nHigh Wind\nTime: 2018-09-20 22:52:00 UTC\n60 mph [Measured]\nNotes: Strong winds measured at 60mph with anemometer""#;
        let event = parser.parse(report).unwrap().unwrap();

        assert!(
            event == Event {
                event_ts: 1537483920000000,
                event_type: EventType::SnReport,
                expires_ts: None,
                fetch_status: None,
                image_uri: None,
                ingest_ts: 0,
                location: Some(Location {
                    county: None,
                    wfo: None,
                    point: Some(Coordinates {
                        lat: 43.112,
                        lon: -94.64
                    }),
                    poly: None
                }),
                md: None,
                outlook: None,
                report: Some(Report {
                    reporter: "Test Human".to_string(),
                    hazard: HazardType::Wind,
                    magnitude: Some(60.0),
                    units: Some(Units::Mph),
                    was_measured: Some(true),
                    report_ts: None
                }),
                text: Some(
                    "Wind reported by Test Human. Strong winds measured at 60mph with anemometer"
                        .to_string()
                ),
                title: "Report: Wind".to_string(),
                valid_ts: None,
                warning: None,
                watch: None
            }
        );
    }

    #[test]
    fn report_should_not_blow_up_with_non_utf8_characters() {
        let parser = ReportParser::new();
        let report = r#"Icon: 43.112000,-94.639999,000,3,5,"Reported By: Test Human\nHigh Wind\nTime: 2018-09-20 22:52:00 UTC\n60 mph [Measured]\nNotes: Strong �������������������������������������������������������������������� measured at 60mph with anemometer""#;
        let event = parser.parse(report);
        assert!(event.is_ok());
    }

    #[test]
    fn report_should_parse_optional_mph() {
        let parser = ReportParser::new();
        let report = r#"Icon: 43.112000,-94.639999,000,3,5,"Reported By: Test Human\nHigh Wind\nTime: 2018-09-20 22:52:00 UTC\n60 mph [Measured]\nNotes: Strong winds measured at 60mph with anemometer""#;
        let parsed_report = parser.parse(report).unwrap().unwrap().report.unwrap();
        assert!(parsed_report.magnitude == Some(60.0));
        assert!(parsed_report.units == Some(Units::Mph));
    }

    #[test]
    fn report_should_parse_optional_measured() {
        let parser = ReportParser::new();
        let report = r#"Icon: 43.112000,-94.639999,000,3,5,"Reported By: Test Human\nHigh Wind\nTime: 2018-09-20 22:52:00 UTC\n60 mph [Measured]\nNotes: Strong winds measured at 60mph with anemometer""#;
        let parsed_report = parser.parse(report).unwrap().unwrap().report.unwrap();
        assert!(parsed_report.was_measured == Some(true));
    }

    #[test]
    fn report_should_parse_optional_size() {
        let parser = ReportParser::new();
        let report = r#"Icon: 47.617706,-111.215248,000,4,4,"Reported By: Test Human\nHail\nTime: 2018-09-20 22:49:29 UTC\nSize: 0.75" (Penny)\nNotes: None""#;
        let parsed_report = parser.parse(report).unwrap().unwrap().report.unwrap();
        assert!(parsed_report.magnitude == Some(0.75));
        assert!(parsed_report.units == Some(Units::Inches));
    }
}
