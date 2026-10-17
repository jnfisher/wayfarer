//! Telnet option negotiation, CP437 and raw capture (docs/plan/02-architecture.md).
//!
//! No sockets here: the gateway owns the socket, feeds bytes to [`Telnet::receive`], writes
//! back the replies, and encodes what it sends with [`Telnet::encode_line`] and
//! [`Telnet::encode_keys`]. State survives chunk boundaries.

mod capture;
pub mod cp437;

pub use capture::Capture;

const IAC: u8 = 255;
const DONT: u8 = 254;
const DO: u8 = 253;
const WONT: u8 = 252;
const WILL: u8 = 251;
const SB: u8 = 250;
const SE: u8 = 240;
const NAWS: u8 = 31;

/// Window size sent for NAWS.
const COLS: u16 = 80;
const ROWS: u16 = 24;

/// What one call to [`Telnet::receive`] produced.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Received {
    /// Decoded text for the terminal (CP437 to Unicode, ANSI left as is).
    pub text: String,
    /// Negotiation bytes to send back to the server.
    pub replies: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Data,
    Iac,
    /// After IAC and a DO/DONT/WILL/WONT; holds the command.
    Option(u8),
    /// Inside a subnegotiation, skipping to IAC SE.
    Sub,
    SubIac,
}

#[derive(Debug)]
pub struct Telnet {
    state: State,
    /// The last reply sent (command, option), so the same one is never sent twice in a row.
    last_reply: Option<(u8, u8)>,
}

impl Default for Telnet {
    fn default() -> Self {
        Self::new()
    }
}

impl Telnet {
    pub fn new() -> Self {
        Self {
            state: State::Data,
            last_reply: None,
        }
    }

    /// Feed bytes from the socket.
    pub fn receive(&mut self, bytes: &[u8]) -> Received {
        let mut out = Received::default();
        for &b in bytes {
            match self.state {
                State::Data => {
                    if b == IAC {
                        self.state = State::Iac;
                    } else {
                        out.text.push(cp437::decode_byte(b));
                    }
                }
                State::Iac => {
                    self.state = match b {
                        IAC => {
                            out.text.push(cp437::decode_byte(IAC));
                            State::Data
                        }
                        DO | DONT | WILL | WONT => State::Option(b),
                        SB => State::Sub,
                        // NOP, GA and the rest carry nothing for us.
                        _ => State::Data,
                    };
                }
                State::Option(cmd) => {
                    self.state = State::Data;
                    if cmd == DO {
                        self.answer_do(b, &mut out.replies);
                    }
                    // WILL: accept silently (server echo, binary). DONT/WONT: no answer.
                }
                State::Sub => {
                    if b == IAC {
                        self.state = State::SubIac;
                    }
                }
                State::SubIac => {
                    self.state = match b {
                        SE => State::Data,
                        IAC => State::Sub,
                        // Malformed: treat as a new command.
                        _ => State::Iac,
                    };
                }
            }
        }
        out
    }

    fn answer_do(&mut self, opt: u8, replies: &mut Vec<u8>) {
        let cmd = if opt == NAWS { WILL } else { WONT };
        if self.last_reply == Some((cmd, opt)) {
            return;
        }
        self.last_reply = Some((cmd, opt));
        replies.extend_from_slice(&[IAC, cmd, opt]);
        if opt == NAWS {
            replies.extend_from_slice(&[IAC, SB, NAWS]);
            for b in [COLS, ROWS].iter().flat_map(|n| n.to_be_bytes()) {
                push_escaped(replies, b);
            }
            replies.extend_from_slice(&[IAC, SE]);
        }
    }

    /// A line for the socket: CP437, then a single CR.
    pub fn encode_line(&self, s: &str) -> Vec<u8> {
        let mut out = Vec::with_capacity(s.len() + 1);
        for c in s.chars() {
            push_escaped(&mut out, cp437::encode_char(c));
        }
        out.push(b'\r');
        out
    }

    /// Keys as given (CP437-encoded, no line ending), with any 255 doubled.
    pub fn encode_keys(&self, s: &str) -> Vec<u8> {
        let mut out = Vec::with_capacity(s.len());
        for c in s.chars() {
            push_escaped(&mut out, cp437::encode_char(c));
        }
        out
    }
}

fn push_escaped(out: &mut Vec<u8>, b: u8) {
    out.push(b);
    if b == IAC {
        out.push(IAC);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OFFERS: [u8; 24] = [
        IAC, WILL, 0, IAC, DO, 0, IAC, WILL, 1, IAC, DONT, 1, IAC, WILL, 3, IAC, DO, 3, IAC, WONT,
        NAWS, IAC, DO, NAWS,
    ];
    const ANSWER: [u8; 15] = [
        IAC, WONT, 0, IAC, WONT, 3, IAC, WILL, NAWS, IAC, SB, NAWS, 0, 80, 0,
    ];

    fn expected_replies() -> Vec<u8> {
        let mut v = ANSWER.to_vec();
        v.extend_from_slice(&[24, IAC, SE]);
        v
    }

    #[test]
    fn mbbsemu_opening_offers() {
        let r = Telnet::new().receive(&OFFERS);
        assert_eq!(r.replies, expected_replies());
        assert_eq!(r.text, "");
    }

    #[test]
    fn split_at_every_point() {
        let mut input = OFFERS.to_vec();
        input.extend_from_slice(b"a\xc9\xdb");
        input.extend_from_slice(&[IAC, IAC, IAC, SB, NAWS, IAC, IAC, IAC, SE, b'z']);
        let whole = Telnet::new().receive(&input);
        assert_eq!(whole.text, "a╔█\u{a0}z");
        for i in 0..=input.len() {
            let mut t = Telnet::new();
            let (a, b) = input.split_at(i);
            let (ra, rb) = (t.receive(a), t.receive(b));
            assert_eq!(ra.text + &rb.text, whole.text, "split {i}");
            let mut replies = ra.replies;
            replies.extend(rb.replies);
            assert_eq!(replies, whole.replies, "split {i}");
        }
        // And one byte at a time.
        let mut t = Telnet::new();
        let (mut text, mut replies) = (String::new(), Vec::new());
        for b in &input {
            let r = t.receive(&[*b]);
            text += &r.text;
            replies.extend(r.replies);
        }
        assert_eq!((text, replies), (whole.text, whole.replies));
    }

    #[test]
    fn refuses_other_dos_and_accepts_wills_silently() {
        let r = Telnet::new().receive(&[IAC, DO, 24, IAC, WILL, 1, IAC, DONT, 5, IAC, WONT, 6]);
        assert_eq!(r.replies, [IAC, WONT, 24]);
    }

    #[test]
    fn same_answer_never_twice_in_a_row() {
        let mut t = Telnet::new();
        assert_eq!(t.receive(&[IAC, DO, 24]).replies, [IAC, WONT, 24]);
        assert!(t.receive(&[IAC, DO, 24]).replies.is_empty());
        assert_eq!(t.receive(&[IAC, DO, 5]).replies, [IAC, WONT, 5]);
        assert_eq!(t.receive(&[IAC, DO, 24]).replies, [IAC, WONT, 24]);
    }

    #[test]
    fn iac_iac_is_a_literal_255() {
        assert_eq!(Telnet::new().receive(&[IAC, IAC]).text, "\u{a0}");
    }

    #[test]
    fn controls_and_ansi_pass_through() {
        let r = Telnet::new().receive(b"\x1b[0;1;31mhi\r\n\r");
        assert_eq!(r.text, "\u{1b}[0;1;31mhi\r\n\r");
    }

    #[test]
    fn subnegotiation_is_skipped() {
        let r = Telnet::new().receive(&[b'a', IAC, SB, 24, 1, 2, 3, IAC, SE, b'b']);
        assert_eq!(r.text, "ab");
    }

    #[test]
    fn lines_end_in_one_cr_and_are_cp437() {
        let t = Telnet::new();
        assert_eq!(t.encode_line("hi"), b"hi\r");
        assert_eq!(t.encode_line("é█€"), [0x82, 0xDB, b'?', b'\r']);
        assert_eq!(t.encode_line(""), b"\r");
    }

    #[test]
    fn keys_are_as_given_with_255_doubled() {
        let t = Telnet::new();
        assert_eq!(t.encode_keys("n\u{1b}"), [b'n', 0x1b]);
        assert_eq!(t.encode_keys("\u{a0}"), [IAC, IAC]);
        assert_eq!(t.encode_line("\u{a0}"), [IAC, IAC, b'\r']);
    }

    #[test]
    fn vendored_captures_decode() {
        let root =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/captures");
        let mut n = 0;
        let mut banner = false;
        for entry in std::fs::read_dir(root).unwrap() {
            let raw = entry.unwrap().path().join("raw.bin");
            let Ok(bytes) = std::fs::read(&raw) else {
                continue;
            };
            let text = Telnet::new().receive(&bytes).text;
            assert!(!text.is_empty(), "{raw:?}");
            banner |= text.contains('╔') && text.contains('█');
            n += 1;
        }
        assert_eq!(n, 12);
        assert!(banner, "no capture has the MBBSEmu banner characters");
    }
}
