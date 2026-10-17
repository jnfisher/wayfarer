//! The incremental parser's behaviour: prompts at the end of a chunk, statline modes, echo,
//! backspaces, sequence numbers. The static table is enough here (no world export).

use std::sync::Arc;

use wayfarer_parse::{Event, Parser, PromptMode, PromptParser, Registry, Value};

fn parser() -> Parser {
    Parser::new(Arc::new(Registry::static_only()))
}

/// A prompt parser, a prompt, and fields it must give.
type Case<'a> = (PromptParser, &'a str, &'a [(&'a str, Value)]);

fn names(evs: &[Event]) -> Vec<&str> {
    evs.iter().map(|e| e.name.as_str()).collect()
}

#[test]
fn a_prompt_is_reported_when_its_chunk_ends() {
    let mut p = parser();
    let evs = p
        .feed("\x1b[79D\x1b[K\x1b[0;33m*Combat Off*\r\n\x1b[79D\x1b[K\x1b[0;37m[HP=35\x1b[0;37m]:");
    assert_eq!(names(&evs), ["combat_off", "prompt"]);
    assert_eq!(evs[1].field("hp"), Some(&Value::Int(35)));
    assert_eq!(evs[1].line, "[HP=35]:");
    // lines: "" before the first CLRLINE, the message, "" after its CR LF, the prompt
    assert_eq!((evs[1].seq, evs[1].line_no), (0, 3));
    // the echo of what we typed completes the line: no second prompt
    p.sent("a rat");
    let evs = p.feed("a rat\r\n");
    assert_eq!(names(&evs), ["echo"]);
    assert_eq!(evs[0].text("cmd"), Some("a rat"));
    assert_eq!((evs[0].seq, evs[0].line_no), (1, 3));
}

#[test]
fn a_prompt_split_across_chunks() {
    let mut p = parser();
    assert!(p.feed("[HP=3").is_empty());
    let evs = p.feed("5]:");
    assert_eq!(names(&evs), ["prompt"]);
    assert!(p.feed("").is_empty(), "reported once");
    assert!(p.finish().is_empty());
}

#[test]
fn a_prompt_cut_before_its_resting_mark_is_reported_again() {
    // The documented exception to split invariance: the game writes this in one piece.
    let mut p = parser();
    let first = p.feed("[HP=12/MA=3]:");
    assert_eq!(first[0].field("state"), Some(&Value::Null));
    let second = p.feed(" (Resting) \r\n");
    assert_eq!(names(&second), ["prompt"]);
    assert_eq!(second[0].field("state"), Some(&"Resting".into()));
}

#[test]
fn piled_prompts_and_a_glued_title() {
    let mut p = parser();
    let evs = p.feed("[HP=35]:[HP=36]:");
    assert_eq!(names(&evs), ["prompt", "prompt"]);
    let evs = p.feed("Slum Street\r\n");
    // static table only: no room names, so the rest is unknown text
    assert_eq!(names(&evs), ["unknown"]);
    assert_eq!(evs[0].line, "Slum Street");
}

#[test]
fn a_wrapped_display_waits_for_its_next_line() {
    let mut p = parser();
    assert!(
        p.feed("Obvious exits: north, closed\r\n").is_empty(),
        "may continue"
    );
    assert!(p.feed("door east,\r\n").is_empty(), "may still continue");
    // a prompt line ends it, before the prompt is complete
    let evs = p.feed("[HP=3");
    assert_eq!(names(&evs), ["room_exits"]);
    assert_eq!(evs[0].text("exits"), Some("north, closed door east,"));
    // seq: the chunk its last line ended in, though it is reported in the next
    assert_eq!((evs[0].line_no, evs[0].seq), (0, 1));
    assert_eq!(names(&p.feed("5]:")), ["prompt"]);
    // text that cannot continue it ends it at once
    let mut p = parser();
    assert_eq!(
        names(&p.feed("Obvious exits: north, south\r\n")),
        ["room_exits"]
    );
}

#[test]
fn statline_modes() {
    let reg = Arc::new(Registry::static_only());
    let cases: [Case<'_>; 4] = [
        (
            PromptParser::new(PromptMode::Off),
            ">",
            &[("form", "off".into())],
        ),
        (
            PromptParser::new(PromptMode::Std),
            "[HP=48/KAI=2]:",
            &[
                ("hp", Value::Int(48)),
                ("mana", Value::Int(2)),
                ("mtype", "KAI".into()),
                ("form", "std".into()),
            ],
        ),
        (
            PromptParser::custom("(%h/%H)%r:").unwrap(),
            "(30/56) (Meditating) :",
            &[
                ("hp", Value::Int(30)),
                ("maxhp", Value::Int(56)),
                ("state", "Meditating".into()),
                ("form", "custom".into()),
            ],
        ),
        (
            PromptParser::new(PromptMode::Auto),
            "[HP=395/395,MA=392/433]:",
            &[
                ("hp", Value::Int(395)),
                ("maxhp", Value::Int(395)),
                ("mana", Value::Int(392)),
                ("form", "generic".into()),
            ],
        ),
    ];
    for (prompt, text, want) in cases {
        let mut p = Parser::with_prompt(reg.clone(), prompt);
        let evs = p.feed(text);
        assert_eq!(names(&evs), ["prompt"], "{text}");
        for (k, v) in want {
            assert_eq!(evs[0].field(k), Some(v), "{text} {k}");
        }
    }
    // a two-line custom prompt reports once, on its last line (the prototype never did: see
    // `Ctx::line`)
    let mut p = Parser::with_prompt(reg, PromptParser::custom("%i%nHP=%h:").unwrap());
    let mut evs = p.feed("Kasimir\r\nHP=30:");
    evs.extend(p.finish());
    assert_eq!(names(&evs), ["prompt"]);
    assert_eq!(evs[0].field("name"), Some(&"Kasimir".into()));
    assert_eq!(evs[0].line_no, 1);
}

#[test]
fn direction_names_with_backspaces() {
    let mut p = parser();
    let mut evs = p.feed("Obvious exits: nJ\x08orth, sD\x08out");
    evs.extend(p.feed("h, uA\x08p\r\n\x1b[79D\x1b[K"));
    evs.extend(p.finish());
    assert_eq!(names(&evs), ["room_exits"]);
    assert_eq!(evs[0].text("exits"), Some("north, south, up"));
}

#[test]
fn seq_is_the_chunk_a_line_ended_in() {
    let mut p = parser();
    assert!(p.feed_at(7, "You gain 13 ").is_empty());
    let evs = p.feed_at(8, "experience.\r\n");
    assert_eq!(names(&evs), ["exp_gain"]);
    assert_eq!(evs[0].seq, 8);
    let evs = p.feed("*Combat Off*\n");
    assert_eq!(evs[0].seq, 9);
}

#[test]
fn every_line_is_an_event() {
    let mut p = parser();
    let mut evs = p.feed("Nothing knows this line\r\n\r\n  \r\nYou are now resting.\r\n");
    evs.extend(p.finish());
    assert_eq!(names(&evs), ["unknown", "rest_start"]);
    assert_eq!(evs[0].cat, "unclassified");
    assert_eq!(evs.iter().map(|e| e.line_no).collect::<Vec<_>>(), [0, 3]);
}

#[test]
fn a_command_sent_during_a_wrapped_display() {
    // The prototype joined its sent-command marker into the display's text.
    let mut p = parser();
    assert!(p.feed("Obvious exits: north,\r\n").is_empty());
    p.sent("n");
    let mut evs = p.feed("south\r\n[HP=35]:n\r\n");
    evs.extend(p.finish());
    assert_eq!(names(&evs), ["room_exits", "prompt", "echo"]);
    assert_eq!(evs[0].text("exits"), Some("north, south"));
    assert_eq!(evs[2].text("cmd"), Some("n"));
}
