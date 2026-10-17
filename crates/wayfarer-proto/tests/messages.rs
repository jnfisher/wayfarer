//! Every message: a fixed JSON string (so a field rename is caught), the round trip, and the
//! examples in the README and `docs/protocol.md`.

use wayfarer_proto::{
    Chunk, ClientKind, DecodeError, PROTOCOL, SessionInfo, SessionState, ToClient, ToGateway,
    protocol_mismatch,
};

fn kasimir(state: SessionState, last_seq: u64) -> SessionInfo {
    SessionInfo {
        id: "kasimir".into(),
        name: "Kasimir".into(),
        server: "bbs.example.org:23".into(),
        state,
        last_seq,
    }
}

fn to_client() -> Vec<(ToClient, &'static str)> {
    vec![
        (
            ToClient::Welcome {
                protocol: 1,
                server_build: "0.1.0".into(),
                sessions: vec![kasimir(SessionState::Realm, 42)],
            },
            r#"{"type":"welcome","protocol":1,"server_build":"0.1.0","sessions":[{"id":"kasimir","name":"Kasimir","server":"bbs.example.org:23","state":"realm","last_seq":42}]}"#,
        ),
        (
            ToClient::Refused {
                reason: "protocol mismatch: client speaks 2, gateway speaks 1".into(),
            },
            r#"{"type":"refused","reason":"protocol mismatch: client speaks 2, gateway speaks 1"}"#,
        ),
        (
            ToClient::SessionList {
                sessions: vec![kasimir(SessionState::Connecting, 0)],
            },
            r#"{"type":"session_list","sessions":[{"id":"kasimir","name":"Kasimir","server":"bbs.example.org:23","state":"connecting","last_seq":0}]}"#,
        ),
        (
            ToClient::SessionState {
                session: "kasimir".into(),
                state: SessionState::Lost,
            },
            r#"{"type":"session_state","session":"kasimir","state":"lost"}"#,
        ),
        (
            ToClient::Output {
                session: "kasimir".into(),
                seq: 43,
                text: "\u{1b}[1;32mObvious exits: north, south\u{1b}[0m\r\n".into(),
            },
            r#"{"type":"output","session":"kasimir","seq":43,"text":"\u001b[1;32mObvious exits: north, south\u001b[0m\r\n"}"#,
        ),
        (
            ToClient::History {
                session: "kasimir".into(),
                from_seq: 41,
                chunks: vec![
                    Chunk {
                        seq: 41,
                        text: "You notice nothing here.\r\n".into(),
                    },
                    Chunk {
                        seq: 42,
                        text: "[HP=30/30]: ".into(),
                    },
                ],
            },
            r#"{"type":"history","session":"kasimir","from_seq":41,"chunks":[{"seq":41,"text":"You notice nothing here.\r\n"},{"seq":42,"text":"[HP=30/30]: "}]}"#,
        ),
        (
            ToClient::Error {
                message: "unknown session \"nobody\"".into(),
            },
            r#"{"type":"error","message":"unknown session \"nobody\""}"#,
        ),
    ]
}

fn to_gateway() -> Vec<(ToGateway, &'static str)> {
    vec![
        (
            ToGateway::Hello {
                protocol: 1,
                client: ClientKind::Ui,
                token: Some("TOKEN".into()),
            },
            r#"{"type":"hello","protocol":1,"client":"ui","token":"TOKEN"}"#,
        ),
        (
            ToGateway::InputLine {
                session: "kasimir".into(),
                text: "look north".into(),
            },
            r#"{"type":"input_line","session":"kasimir","text":"look north"}"#,
        ),
        (
            ToGateway::InputKeys {
                session: "kasimir".into(),
                bytes: vec![13],
            },
            r#"{"type":"input_keys","session":"kasimir","bytes":[13]}"#,
        ),
        (
            ToGateway::HistoryRequest {
                session: "kasimir".into(),
                after_seq: Some(42),
                before_seq: None,
                max_chunks: 2000,
            },
            r#"{"type":"history_request","session":"kasimir","after_seq":42,"max_chunks":2000}"#,
        ),
        (
            ToGateway::Subscribe {
                sessions: Some(vec!["kasimir".into()]),
            },
            r#"{"type":"subscribe","sessions":["kasimir"]}"#,
        ),
    ]
}

#[test]
fn to_client_fixed_json_and_round_trip() {
    let cases = to_client();
    let types: Vec<_> = cases.iter().map(|(msg, _)| msg.type_name()).collect();
    assert_eq!(types, ToClient::TYPES, "one case per variant, in order");
    for (msg, json) in cases {
        assert_eq!(msg.to_json(), json);
        assert_eq!(ToClient::from_json(json).unwrap(), msg);
        assert_eq!(ToClient::from_json(&msg.to_json()).unwrap(), msg);
    }
}

#[test]
fn to_gateway_fixed_json_and_round_trip() {
    let cases = to_gateway();
    let types: Vec<_> = cases.iter().map(|(msg, _)| msg.type_name()).collect();
    assert_eq!(types, ToGateway::TYPES, "one case per variant, in order");
    for (msg, json) in cases {
        assert_eq!(msg.to_json(), json);
        assert_eq!(ToGateway::from_json(json).unwrap(), msg);
        assert_eq!(ToGateway::from_json(&msg.to_json()).unwrap(), msg);
    }
}

#[test]
fn optional_fields_may_be_left_out() {
    let cases = [
        (
            ToGateway::Hello {
                protocol: 1,
                client: ClientKind::Brain,
                token: None,
            },
            r#"{"type":"hello","protocol":1,"client":"brain"}"#,
        ),
        (
            ToGateway::HistoryRequest {
                session: "kasimir".into(),
                after_seq: None,
                before_seq: Some(43),
                max_chunks: 2000,
            },
            r#"{"type":"history_request","session":"kasimir","before_seq":43,"max_chunks":2000}"#,
        ),
        (
            ToGateway::HistoryRequest {
                session: "kasimir".into(),
                after_seq: None,
                before_seq: None,
                max_chunks: 100,
            },
            r#"{"type":"history_request","session":"kasimir","max_chunks":100}"#,
        ),
        (
            ToGateway::Subscribe { sessions: None },
            r#"{"type":"subscribe"}"#,
        ),
    ];
    for (msg, json) in cases {
        assert_eq!(msg.to_json(), json);
        assert_eq!(ToGateway::from_json(json).unwrap(), msg);
    }
    // An explicit null reads the same as a missing field.
    assert_eq!(
        ToGateway::from_json(r#"{"type":"subscribe","sessions":null}"#).unwrap(),
        ToGateway::Subscribe { sessions: None }
    );
}

#[test]
fn unknown_type_is_a_typed_error() {
    assert!(matches!(
        ToGateway::from_json(r#"{"type":"teleport","session":"kasimir"}"#),
        Err(DecodeError::UnknownType(kind)) if kind == "teleport"
    ));
    // A gateway → client type sent the wrong way is unknown too.
    assert!(matches!(
        ToGateway::from_json(r#"{"type":"output","session":"kasimir","seq":1,"text":""}"#),
        Err(DecodeError::UnknownType(kind)) if kind == "output"
    ));
    assert!(matches!(
        ToClient::from_json(r#"{"type":"hello","protocol":1,"client":"ui"}"#),
        Err(DecodeError::UnknownType(kind)) if kind == "hello"
    ));
    assert!(matches!(
        ToClient::from_json(r#"{"type":7}"#),
        Err(DecodeError::UnknownType(kind)) if kind == "7"
    ));
}

#[test]
fn malformed_messages_are_typed_errors() {
    assert!(matches!(
        ToGateway::from_json("not json"),
        Err(DecodeError::NotJson(_))
    ));
    assert!(matches!(
        ToGateway::from_json("[1, 2]"),
        Err(DecodeError::NotJson(_))
    ));
    assert!(matches!(
        ToGateway::from_json(r#"{"session":"kasimir"}"#),
        Err(DecodeError::MissingType)
    ));
    // A known type with a missing or wrongly typed field.
    assert!(matches!(
        ToGateway::from_json(r#"{"type":"input_line","session":"kasimir"}"#),
        Err(DecodeError::Invalid { kind, .. }) if kind == "input_line"
    ));
    assert!(matches!(
        ToGateway::from_json(r#"{"type":"input_keys","session":"kasimir","bytes":[256]}"#),
        Err(DecodeError::Invalid { kind, .. }) if kind == "input_keys"
    ));
    assert!(matches!(
        ToClient::from_json(r#"{"type":"session_state","session":"kasimir","state":"asleep"}"#),
        Err(DecodeError::Invalid { kind, .. }) if kind == "session_state"
    ));
}

#[test]
fn protocol_mismatch_names_both_numbers() {
    assert_eq!(protocol_mismatch(PROTOCOL), None);
    assert_eq!(
        protocol_mismatch(PROTOCOL + 1).unwrap(),
        format!(
            "protocol mismatch: client speaks {}, gateway speaks {PROTOCOL}",
            PROTOCOL + 1
        )
    );
}

#[test]
fn every_example_is_in_the_docs() {
    let readme = include_str!("../README.md");
    let protocol = include_str!("../../../docs/protocol.md");
    let examples = to_client()
        .into_iter()
        .map(|(_, json)| json)
        .chain(to_gateway().into_iter().map(|(_, json)| json));
    for json in examples {
        assert!(readme.contains(json), "README.md lacks {json}");
        assert!(protocol.contains(json), "docs/protocol.md lacks {json}");
    }
}
