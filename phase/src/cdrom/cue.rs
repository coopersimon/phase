use crate::utils::bcd::from_bcd;
use super::DriveLoc;
use regex::Regex;

pub struct CueFile {
    pub tracks: Vec<Track>
}

impl CueFile {
    pub fn parse_from_str(str: &str) -> Result<CueFile, ParseError> {
        let file_regex = Regex::new(r#"FILE\s+"(.*?)"\s+BINARY"#).unwrap();
        let track_regex = Regex::new(r"\s*TRACK\s+([0-9][0-9])\s+([A-Z0-9/]+)").unwrap();
        let index_regex = Regex::new(r"\s*INDEX\s+([0-9][0-9])\s+([0-9][0-9]):([0-9][0-9]):([0-9][0-9])").unwrap();
        let mut lines = str.lines().peekable();
        let mut tracks = Vec::new();
        while let Some(line) = lines.next() {
            if line.is_empty() {
                continue;
            }
            let Some(file_captures) = file_regex.captures(line) else {
                return Err(ParseError::InvalidFileMarker);
            };
            let Some(file_name) = file_captures.get(1) else {
                return Err(ParseError::NoFileName);
            };
            let Some(track_line) = lines.next() else {
                return Err(ParseError::InvalidTrackMarker);
            };
            let Some(track_captures) = track_regex.captures(track_line) else {
                return Err(ParseError::InvalidTrackMarker);
            };
            let Some(track_num_str) = track_captures.get(1) else {
                return Err(ParseError::InvalidTrackMarker);
            };
            let Some(track_type_str) = track_captures.get(2) else {
                return Err(ParseError::InvalidTrackMarker);
            };
            let Ok(track_num) = track_num_str.as_str().parse() else {
                return Err(ParseError::InvalidTrackMarker);
            };
            let track_type = match track_type_str.as_str() {
                "MODE2/2352" => TrackType::DataMode2,
                "AUDIO" => TrackType::Audio,
                _ => return Err(ParseError::InvalidTrackMarker),
            };

            let mut track_indices = Vec::new();
            while let Some(index_line) = lines.peek() {
                if !index_line.trim_start().starts_with("INDEX") {
                    break;
                }
                let Some(index_captures) = index_regex.captures(index_line) else {
                    return Err(ParseError::InvalidIndexMarker);
                };
                let Some(index_str) = index_captures.get(1) else {
                    return Err(ParseError::InvalidIndexMarker);
                };
                let Some(minute_str) = index_captures.get(2) else {
                    return Err(ParseError::InvalidIndexMarker);
                };
                let Some(second_str) = index_captures.get(3) else {
                    return Err(ParseError::InvalidIndexMarker);
                };
                let Some(sector_str) = index_captures.get(4) else {
                    return Err(ParseError::InvalidIndexMarker);
                };
                let index = Index {
                    num: index_str.as_str().parse().expect("invalid index"),
                    start: DriveLoc {
                        minute: from_bcd(minute_str.as_str().parse().expect("invalid minute")).unwrap(),
                        second: from_bcd(second_str.as_str().parse().expect("invalid second")).unwrap(),
                        sector: from_bcd(sector_str.as_str().parse().expect("invalid sector")).unwrap(),
                    }
                };
                track_indices.push(index);
                let _ = lines.next();
            }
            let track = Track {
                num: track_num,
                file_name: file_name.as_str().to_string(),
                track_type: track_type,
                indices: track_indices
            };
            tracks.push(track);
        }
        Ok(CueFile { tracks })
    }
}

pub struct Track {
    pub num: usize,
    pub file_name: String,
    pub track_type: TrackType,
    pub indices: Vec<Index>,
}

pub enum TrackType {
    Audio,
    DataMode2
}

pub struct Index {
    pub num: usize,
    pub start: DriveLoc,
}

#[derive(Debug)]
pub enum ParseError {
    InvalidFileMarker,
    NoFileName,
    InvalidTrackMarker,
    InvalidIndexMarker,
}