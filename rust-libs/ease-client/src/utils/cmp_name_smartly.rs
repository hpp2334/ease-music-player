

use unicode_segmentation::UnicodeSegmentation;

enum NameToken {
    Other(String),
    Num(String),
}

fn parse_name(s: &str) -> Vec<NameToken> {
    #[derive(Debug, PartialEq, Eq)]
    enum LastState {
        None,
        Other,
        Num,
    }

    let mut buf = String::new();
    let mut ret: Vec<NameToken> = Default::default();
    let mut last_state = LastState::None;

    let mut flush = |last_state: &LastState, buf: &mut String| {
        if !buf.is_empty() {
            if last_state == &LastState::Num {
                ret.push(NameToken::Num(buf.to_string()));
            } else if last_state == &LastState::Other {
                ret.push(NameToken::Other(buf.to_string()));
            }
            buf.clear();
        }
    };

    for c in s.graphemes(false) {
        if c.is_ascii() && c.chars().next().unwrap().is_digit(10) {
            if last_state != LastState::Num {
                flush(&last_state, &mut buf);
            }
            last_state = LastState::Num;
            buf += c;
        } else {
            if last_state != LastState::Other {
                flush(&last_state, &mut buf);
            }
            last_state = LastState::Other;
            buf += c;
        }
    }
    flush(&last_state, &mut buf);

    ret
}

fn cmp_name_token(lhs: &NameToken, rhs: &NameToken) -> std::cmp::Ordering {
    match (lhs, rhs) {
        (NameToken::Other(lhs), NameToken::Other(rhs)) => lhs.cmp(rhs),
        (NameToken::Other(_), NameToken::Num(_)) => std::cmp::Ordering::Greater,
        (NameToken::Num(_), NameToken::Other(_)) => std::cmp::Ordering::Less,
        (NameToken::Num(lhs), NameToken::Num(rhs)) => {
            let x = lhs.parse::<usize>();
            let y = rhs.parse::<usize>();
            if x.is_ok() && y.is_ok() {
                return x.unwrap().cmp(&y.unwrap());
            }
            return lhs.cmp(rhs);
        }
    }
}

fn cmp_name_tokens(lhs: &Vec<NameToken>, rhs: &Vec<NameToken>) -> std::cmp::Ordering {
    let n = lhs.len().min(rhs.len());

    for i in 0..n {
        let c = cmp_name_token(&lhs[i], &rhs[i]);
        if !c.is_eq() {
            return c;
        }
    }
    return lhs.len().cmp(&rhs.len());
}

pub fn cmp_name_smartly(lhs: &str, rhs: &str) -> std::cmp::Ordering {
    let lhs = parse_name(lhs);
    let rhs = parse_name(rhs);

    cmp_name_tokens(&lhs, &rhs)
}

#[cfg(test)]
mod test {
    use std::cmp::Ordering;

    use super::cmp_name_smartly;

    #[test]
    fn cmp_simple_num() {
        assert_eq!(cmp_name_smartly("1", "10"), Ordering::Less);
        assert_eq!(cmp_name_smartly("2", "10"), Ordering::Less);
        assert_eq!(cmp_name_smartly("2", "2"), Ordering::Equal);
        assert_eq!(cmp_name_smartly("02", "2"), Ordering::Equal);
    }

    #[test]
    fn cmp_simple_bracket_num() {
        assert_eq!(cmp_name_smartly("[1]", "[10]"), Ordering::Less);
        assert_eq!(cmp_name_smartly("[2]", "[10]"), Ordering::Less);
        assert_eq!(cmp_name_smartly("[2]", "[2]"), Ordering::Equal);
        assert_eq!(cmp_name_smartly("[02]", "[2]"), Ordering::Equal);
    }
}
