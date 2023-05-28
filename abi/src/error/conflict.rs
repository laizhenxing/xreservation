use chrono::{DateTime, Utc};
use regex::Regex;
use std::{collections::HashMap, convert::Infallible, str::FromStr};
/// TODO: write a parser
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReservationConflictInfo {
    Parsed(ReservationConflict),
    Unparsed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReservationConflict {
    pub old: ReservationWindow,
    pub new: ReservationWindow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReservationWindow {
    /// resource id
    pub rid: String,
    /// start time
    pub start: DateTime<Utc>,
    /// end time
    pub end: DateTime<Utc>,
}

struct ParsedInfo {
    old: HashMap<String, String>,
    new: HashMap<String, String>,
}

impl FromStr for ReservationConflictInfo {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(conflict) = s.parse() {
            Ok(Self::Parsed(conflict))
        } else {
            Ok(Self::Unparsed(s.to_string()))
        }
    }
}

impl FromStr for ReservationConflict {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ParsedInfo::from_str(s)?.try_into()
    }
}

impl TryFrom<ParsedInfo> for ReservationConflict {
    type Error = ();

    fn try_from(info: ParsedInfo) -> Result<Self, Self::Error> {
        Ok(Self {
            old: info.old.try_into()?,
            new: info.new.try_into()?,
        })
    }
}

impl TryFrom<HashMap<String, String>> for ReservationWindow {
    type Error = ();

    fn try_from(info: HashMap<String, String>) -> Result<Self, Self::Error> {
        // "timespan": "\"2023-01-02 17:10:10+00\",\"2023-01-05 17:10:10+00\""
        // 把 " 过滤
        let timespan_str = info.get("timespan").ok_or(())?.replace('"', "");
        let mut split = timespan_str.splitn(2, ',');
        let start = parse_time(split.next().ok_or(())?)?;
        println!("{:?}", start);
        let end = parse_time(split.next().ok_or(())?)?;
        Ok(Self {
            rid: info.get("resource_id").ok_or(())?.to_string(),
            start,
            end,
        })
    }
}

/// Key (resource_id, timespan)=(test-resource, [\"2023-01-02 17:10:10+00\",\"2023-01-05 17:10:10+00\")) conflicts with existing key (resource_id, timespan)=(test-resource, [\"2023-01-01 17:10:10+00\",\"2023-01-04 17:10:10+00\")).
impl FromStr for ParsedInfo {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let re = Regex::new(r#"\((?P<k1>[a-zA-Z0-9_-]+)\s*,\s*(?P<k2>[a-zA-Z0-9_-]+)\)=\((?P<v1>[a-zA-Z0-9_-]+)\s*,\s*\[(?P<v2>[^\)\]]+)"#).unwrap();
        let mut maps = vec![];
        for cap in re.captures_iter(s) {
            let mut map = HashMap::new();
            map.insert(cap["k1"].to_string(), cap["v1"].to_string());
            map.insert(cap["k2"].to_string(), cap["v2"].to_string());
            maps.push(map);
        }
        if maps.len() != 2 {
            return Err(());
        }
        Ok(Self {
            new: maps[0].clone(),
            old: maps[1].clone(),
        })
    }
}

fn parse_time(s: &str) -> Result<DateTime<Utc>, ()> {
    Ok(DateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%#z")
        .map_err(|_| ())?
        .with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: &str = "Key (resource_id, timespan)=(test-resource, [\"2023-01-02 17:10:10+00\",\"2023-01-05 17:10:10+00\")) conflicts with existing key (resource_id, timespan)=(test-resource, [\"2023-01-01 17:10:10+00\",\"2023-01-04 17:10:10+00\")).";

    #[test]
    fn from_str_parse_into_parsed_info_should_work() {
        let info = ParsedInfo::from_str(S).unwrap();
        assert_eq!(
            info.new.get("resource_id").unwrap().clone(),
            "test-resource".to_string()
        );
        assert_eq!(
            info.old.get("resource_id").unwrap().clone(),
            "test-resource".to_string()
        )
    }

    #[test]
    fn from_str_parse_into_conflict_window_should_work() {
        let info = ParsedInfo::from_str(S).unwrap();
        let new_window = ReservationWindow::try_from(info.new).unwrap();
        let old_window = ReservationWindow::try_from(info.old).unwrap();
        assert_eq!(new_window.rid, "test-resource".to_string());
        assert_eq!(
            new_window.start,
            parse_time("2023-01-02 17:10:10+00").unwrap()
        );
        assert_eq!(
            new_window.end,
            parse_time("2023-01-05 17:10:10+00").unwrap()
        );

        assert_eq!(old_window.rid, "test-resource".to_string());
        assert_eq!(
            old_window.start,
            parse_time("2023-01-01 17:10:10+00").unwrap()
        );
        assert_eq!(
            old_window.end,
            parse_time("2023-01-04 17:10:10+00").unwrap()
        );
    }

    #[test]
    fn try_from_parse_info_to_reservation_conflict_should_work() {
        let info = ParsedInfo::from_str(S).unwrap();
        let rsvp_cft = ReservationConflict::try_from(info).unwrap();
        assert_eq!(rsvp_cft.new.rid, "test-resource".to_string());

        assert_eq!(
            rsvp_cft.new.start,
            parse_time("2023-01-02 17:10:10+00").unwrap()
        );
        assert_eq!(
            rsvp_cft.new.end,
            parse_time("2023-01-05 17:10:10+00").unwrap()
        );

        assert_eq!(rsvp_cft.old.rid, "test-resource".to_string());
        assert_eq!(
            rsvp_cft.old.start,
            parse_time("2023-01-01 17:10:10+00").unwrap()
        );
        assert_eq!(
            rsvp_cft.old.end,
            parse_time("2023-01-04 17:10:10+00").unwrap()
        );
    }

    #[test]
    fn from_str_parse_into_revervation_conflict_info_should_work() {
        let rsvp_cft_info = ReservationConflictInfo::from_str(S).unwrap();
        assert!(matches!(rsvp_cft_info, ReservationConflictInfo::Parsed(..)));
    }
}
