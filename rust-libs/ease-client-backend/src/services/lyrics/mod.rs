// This is a fork version of [https://github.com/waylyrics/lrc-nom](https://github.com/waylyrics/lrc-nom)

use std::time::Duration;

use ease_client_shared::backends::lyric::Lyrics;
use nom::IResult;
use nom::{
    bytes::complete::{tag, take_until},
    multi::many1,
    sequence::tuple,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LrcParseError {
    #[error("No tag was found in non-empty line {0}: {1}")]
    NoTagInNonEmptyLine(usize, String),
    #[error("Invalid timestamp format in line {0}")]
    InvalidTimestamp(usize),
    #[error("Invalid offset format in line {0}")]
    InvalidOffset(usize),
}

fn parse_tags(line: &str, i: usize) -> Result<(&str, Vec<(&str, &str)>), LrcParseError> {
    let res: IResult<&str, Vec<(&str, &str, &str, &str, &str)>> = many1(tuple((
        tag("["),
        take_until(":"),
        tag(":"),
        take_until("]"),
        tag("]"),
    )))(line);

    if res.is_err() {
        tracing::error!("parse_tags error {}", res.unwrap_err());
        return Err(LrcParseError::NoTagInNonEmptyLine(i, line.to_string()));
    }
    let (text, tags) = res.unwrap();
    let tags = tags
        .into_iter()
        .map(|(_left_sq, attr, _semicon, content, _right_sq)| (attr, content))
        .collect();
    Ok((text, tags))
}

pub(crate) fn parse_lrc(lyric: impl Into<String>) -> Result<Lyrics, LrcParseError> {
    let lyric_lines: String = lyric.into();
    let lyric_lines = lyric_lines.trim_start_matches("\u{feff}");
    let lyric_lines: Vec<&str> = lyric_lines
        .split("\n")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    let mut lyrics = Lyrics {
        metdata: Default::default(),
        lines: Default::default(),
    };

    for (i, line) in lyric_lines.into_iter().enumerate() {
        let (text, tags) = parse_tags(line, i)?;
        match tags[0] {
            // `[:]` is considered as comment line
            ("", "") => continue,
            (attr, content) => match attr.trim() {
                "ar" => lyrics.metdata.artist = content.trim().to_string(),
                "al" => lyrics.metdata.album = content.trim().to_string(),
                "ti" => lyrics.metdata.title = content.trim().to_string(),
                "au" => lyrics.metdata.lyricist = content.trim().to_string(),
                "length" => lyrics.metdata.length = content.trim().to_string(),
                "by" => lyrics.metdata.author = content.trim().to_string(),
                "offset" => lyrics.metdata.offset = content.trim().to_string(),
                _minute if _minute.parse::<u64>().is_ok() => {
                    for (minute, second) in tags.into_iter() {
                        let minute = minute
                            .trim()
                            .parse::<u64>()
                            .map_err(|_| LrcParseError::InvalidTimestamp(i))?;
                        let second = second.replace(":", ".");
                        let second: Vec<&str> = second.split(".").collect();
                        let milliseconds = if second.len() < 2 {
                            0
                        } else {
                            let s = second[1]
                                .trim()
                                .parse::<u64>()
                                .map_err(|_| LrcParseError::InvalidTimestamp(i))?;
                            if second[1].len() == 1 {
                                s * 100
                            } else if second[1].len() == 2 {
                                s * 10
                            } else {
                                s
                            }
                        };
                        let sec = second[0]
                            .trim()
                            .parse::<u64>()
                            .map_err(|_| LrcParseError::InvalidTimestamp(i))?;

                        let minute = Duration::from_secs(minute * 60);
                        let sec = Duration::from_secs(sec);
                        let milliseconds = Duration::from_millis(milliseconds);
                        let time = minute + sec + milliseconds;

                        let text = text.trim().to_string();
                        if !text.is_empty() {
                            lyrics.lines.push((time, text.to_string()));
                        }
                    }
                }
                _ => (), // ignores unrecognized tags
            },
        };
    }

    Ok(lyrics)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::parse_lrc;

    #[test]
    fn lrc_1() {
        let res = parse_lrc(
            "[00:12.00]Line 1 lyrics
        [00:17.20]Line 2 lyrics
        
        [00:21.10][00:45.10]Repeating lyrics (e.g. chorus)",
        );
        assert!(res.is_ok());
        let res = res.unwrap();
        let mut line = res.lines.into_iter();
        assert_eq!(
            line.next(),
            Some((Duration::from_secs(12), "Line 1 lyrics".to_string()))
        );
        assert_eq!(
            line.next(),
            Some((
                Duration::from_secs(17) + Duration::from_millis(200),
                "Line 2 lyrics".to_string()
            ))
        );
        assert_eq!(
            line.next(),
            Some((
                Duration::from_secs(21) + Duration::from_millis(100),
                "Repeating lyrics (e.g. chorus)".to_string()
            ))
        );
        assert_eq!(
            line.next(),
            Some((
                Duration::from_secs(45) + Duration::from_millis(100),
                "Repeating lyrics (e.g. chorus)".to_string()
            ))
        );
        assert_eq!(line.next(), None);
    }

    #[test]
    fn lrc_2() {
        let res = parse_lrc(
            "[by:Arctime]
            [00:01.00]A
            [00:02.00]B
            [00:04.00]C
            [00:02.12][00:03.67] ",
        );
        assert!(res.is_ok());
        let res = res.unwrap();
        let mut line = res.lines.into_iter();
        assert_eq!(line.next(), Some((Duration::from_secs(1), "A".to_string())));
        assert_eq!(line.next(), Some((Duration::from_secs(2), "B".to_string())));
        assert_eq!(line.next(), Some((Duration::from_secs(4), "C".to_string())));
        assert_eq!(line.next(), None);
    }

    #[test]
    fn lrc_3() {
        let res = parse_lrc("[ 00 : 01 . 00 ]A");
        assert!(res.is_ok());
        let res = res.unwrap();
        let mut line = res.lines.into_iter();
        assert_eq!(line.next(), Some((Duration::from_secs(1), "A".to_string())));
        assert_eq!(line.next(), None);
    }
}
