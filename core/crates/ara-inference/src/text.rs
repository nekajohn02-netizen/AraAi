//! Engine-independent streaming text utilities.

/// Reassembles UTF-8 from token byte pieces. A single token may carry only
/// part of a multi-byte character (common for Arabic and emoji), so bytes are
/// buffered until a complete character is available. Invalid sequences become
/// U+FFFD instead of being dropped silently.
#[derive(Debug, Default)]
pub struct Utf8Stream {
    buf: Vec<u8>,
}

impl Utf8Stream {
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends bytes and returns all text that is now complete.
    pub fn push(&mut self, bytes: &[u8]) -> String {
        self.buf.extend_from_slice(bytes);
        let mut out = String::new();
        loop {
            match std::str::from_utf8(&self.buf) {
                Ok(s) => {
                    out.push_str(s);
                    self.buf.clear();
                    return out;
                }
                Err(e) => {
                    let valid = e.valid_up_to();
                    // SAFETY-free: the prefix was just validated.
                    out.push_str(std::str::from_utf8(&self.buf[..valid]).unwrap());
                    match e.error_len() {
                        // Incomplete trailing sequence: wait for more bytes.
                        None => {
                            self.buf.drain(..valid);
                            return out;
                        }
                        Some(n) => {
                            out.push('\u{FFFD}');
                            self.buf.drain(..valid + n);
                        }
                    }
                }
            }
        }
    }

    /// Flushes any incomplete trailing bytes as U+FFFD.
    pub fn finish(&mut self) -> String {
        if self.buf.is_empty() {
            String::new()
        } else {
            self.buf.clear();
            "\u{FFFD}".to_string()
        }
    }
}

/// Holds back streamed text that might be the start of a stop sequence, so a
/// stop string is never shown to the user (not even partially).
#[derive(Debug)]
pub struct StopFilter {
    stops: Vec<String>,
    pending: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct StopOutput {
    /// Text that is safe to emit now.
    pub emit: String,
    /// True when a stop sequence was found; generation must end.
    pub stopped: bool,
}

impl StopFilter {
    pub fn new(stops: &[String]) -> Self {
        Self {
            stops: stops.iter().filter(|s| !s.is_empty()).cloned().collect(),
            pending: String::new(),
        }
    }

    pub fn push(&mut self, text: &str) -> StopOutput {
        self.pending.push_str(text);
        // Earliest full match wins.
        if let Some(pos) = self
            .stops
            .iter()
            .filter_map(|s| self.pending.find(s.as_str()))
            .min()
        {
            let emit = self.pending[..pos].to_string();
            self.pending.clear();
            return StopOutput {
                emit,
                stopped: true,
            };
        }
        // Keep the longest suffix of `pending` that is a prefix of a stop.
        let hold = self
            .stops
            .iter()
            .map(|s| longest_suffix_prefix(&self.pending, s))
            .max()
            .unwrap_or(0);
        let cut = self.pending.len() - hold;
        let emit = self.pending[..cut].to_string();
        self.pending.drain(..cut);
        StopOutput {
            emit,
            stopped: false,
        }
    }

    /// Releases held-back text at the end of generation (no stop matched).
    pub fn finish(&mut self) -> String {
        std::mem::take(&mut self.pending)
    }
}

/// Length in bytes of the longest proper suffix of `text` that is a prefix of
/// `stop`, respecting char boundaries.
fn longest_suffix_prefix(text: &str, stop: &str) -> usize {
    let max = text.len().min(stop.len().saturating_sub(1));
    (1..=max)
        .rev()
        .find(|&n| {
            let start = text.len() - n;
            text.is_char_boundary(start)
                && stop.is_char_boundary(n)
                && stop.as_bytes()[..n] == text.as_bytes()[start..]
        })
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_reassembles_split_arabic_characters() {
        let word = "مرحبا"; // each letter is 2 bytes
        let bytes = word.as_bytes();
        let mut s = Utf8Stream::new();
        let mut out = String::new();
        for b in bytes {
            out.push_str(&s.push(std::slice::from_ref(b)));
        }
        out.push_str(&s.finish());
        assert_eq!(out, word);
    }

    #[test]
    fn utf8_reassembles_four_byte_emoji_across_pieces() {
        let e = "🙂".as_bytes();
        let mut s = Utf8Stream::new();
        assert_eq!(s.push(&e[..1]), "");
        assert_eq!(s.push(&e[1..3]), "");
        assert_eq!(s.push(&e[3..]), "🙂");
    }

    #[test]
    fn utf8_replaces_invalid_bytes_and_continues() {
        let mut s = Utf8Stream::new();
        assert_eq!(s.push(b"a\xFFb"), "a\u{FFFD}b");
        assert_eq!(s.push(&[0xD9]), "");
        assert_eq!(s.finish(), "\u{FFFD}");
    }

    fn run(stops: &[&str], chunks: &[&str]) -> (String, bool) {
        let stops: Vec<String> = stops.iter().map(|s| s.to_string()).collect();
        let mut f = StopFilter::new(&stops);
        let mut out = String::new();
        for c in chunks {
            let o = f.push(c);
            out.push_str(&o.emit);
            if o.stopped {
                return (out, true);
            }
        }
        out.push_str(&f.finish());
        (out, false)
    }

    #[test]
    fn stop_sequence_split_across_chunks_is_never_emitted() {
        assert_eq!(
            run(&["<|end|>"], &["Hello <|", "en", "d|> trailing"]),
            ("Hello ".to_string(), true)
        );
    }

    #[test]
    fn partial_match_that_diverges_is_released() {
        assert_eq!(
            run(&["<|end|>"], &["a <|e", "x"]),
            ("a <|ex".to_string(), false)
        );
    }

    #[test]
    fn earliest_stop_wins_and_no_stops_passes_through() {
        assert_eq!(run(&["B", "A"], &["xxAyyB"]), ("xx".to_string(), true));
        assert_eq!(run(&[], &["abc", "def"]), ("abcdef".to_string(), false));
    }

    #[test]
    fn arabic_stop_sequence_respects_char_boundaries() {
        assert_eq!(
            run(&["انتهى"], &["النص ان", "تهى بعده"]),
            ("النص ".to_string(), true)
        );
    }
}
