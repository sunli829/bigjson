use std::borrow::Cow;

use crate::ParseJsonPointerError;

pub(crate) struct JsonPointerParser<'a> {
    input: &'a [u8],
}

impl<'a> JsonPointerParser<'a> {
    #[inline]
    pub(crate) fn new(input: &'a str) -> Self {
        Self {
            input: input.as_bytes(),
        }
    }
}

impl<'a> Iterator for JsonPointerParser<'a> {
    type Item = Result<String, ParseJsonPointerError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.input.is_empty() {
            return None;
        }

        if self.input[0] != b'/' {
            return Some(Err(ParseJsonPointerError));
        }
        self.input = &self.input[1..];

        match memchr::memchr(b'/', self.input) {
            Some(idx) => {
                let segment = parse_segment(&self.input[..idx]);
                self.input = &self.input[idx..];
                Some(Ok(segment.into_owned()))
            }
            None => {
                let segment = parse_segment(self.input);
                self.input = &[];
                Some(Ok(segment.into_owned()))
            }
        }
    }
}

#[inline]
fn is_escape_char(ch: u8) -> bool {
    ch == b'0' || ch == b'1'
}

fn parse_segment(value: &[u8]) -> Cow<str> {
    fn find_escape(value: &[u8]) -> Option<usize> {
        let mut p = 0;
        let len = value.len();

        while p < len {
            match memchr::memchr(b'~', &value[p..]) {
                Some(idx) if p + idx + 1 < len && is_escape_char(value[p + idx + 1]) => {
                    return Some(p + idx);
                }
                Some(idx) => p += idx + 1,
                None => return None,
            }
        }

        None
    }

    match find_escape(value) {
        Some(idx) => {
            let mut s = Vec::new();
            let mut i = idx + 2;
            let len = value.len();

            s.extend_from_slice(&value[..idx]);
            match value[idx + 1] {
                b'0' => s.push(b'~'),
                b'1' => s.push(b'/'),
                _ => unreachable!(),
            }

            while i < len {
                let ch = value[i];
                match ch {
                    b'~' if i + 1 < len => {
                        match value[i + 1] {
                            b'0' => s.push(b'~'),
                            b'1' => s.push(b'/'),
                            c => {
                                s.push(b'~');
                                s.push(c);
                            }
                        }
                        i += 2;
                    }
                    _ => {
                        s.push(ch);
                        i += 1;
                    }
                }
            }

            unsafe { String::from_utf8_unchecked(s).into() }
        }
        None => unsafe { std::str::from_utf8_unchecked(value).into() },
    }
}

pub(crate) fn parse_json_pointer(input: &str) -> Result<Vec<String>, ParseJsonPointerError> {
    let parser = JsonPointerParser::new(input);
    let mut segments = Vec::new();

    for res in parser {
        let segment = res?;
        segments.push(segment);
    }

    Ok(segments)
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! segments {
        ($($value:literal),*) => {
            &[$($value.to_string()),*]
        }
    }

    fn check(input: &str, segments: &[String]) {
        assert_eq!(parse_json_pointer(input).unwrap(), segments);
    }

    #[test]
    fn test_parser() {
        check("", segments!());
        check("/foo", segments!("foo"));
        check("/foo/0", segments!("foo", "0"));
        check("/", segments!(""));
        check("/a~1b", segments!("a/b"));
        check("/c%d", segments!("c%d"));
        check("/e^f", segments!("e^f"));
        check("/g|h", segments!("g|h"));
        check("/ ", segments!(" "));
        check("/m~0n", segments!("m~n"));
        check("/a~c/~1bc/~2d", segments!("a~c", "/bc", "~2d"));
    }
}
