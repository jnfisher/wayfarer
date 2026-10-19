//! Every message: a fixed JSON string (so a field rename is caught), the round trip, and the
//! examples in the README and `docs/protocol.md`.

use wayfarer_proto::{
    CharacterState, Chunk, ClientKind, DecodeError, Family, PROTOCOL, ParseCounts, SessionInfo,
    SessionState, Stats, ToClient, ToGateway, WorldPackInfo, protocol_mismatch,
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

/// The brain's messages (the same JSON whichever way they go), in `ToGateway` order.
fn brain_messages() -> Vec<(ToGateway, &'static str)> {
    let fields = serde_json::json!({"dmg": "1", "target": "fat giant rat", "verb": "punch"});
    vec![
        (
            ToGateway::Event {
                session: "kasimir".into(),
                seq: 43,
                line_no: 88,
                name: "you_hit".into(),
                cat: "combat".into(),
                fields: fields.as_object().unwrap().clone(),
                line: "You punch fat giant rat for 1 damage!".into(),
            },
            r#"{"type":"event","session":"kasimir","seq":43,"line_no":88,"name":"you_hit","cat":"combat","fields":{"dmg":"1","target":"fat giant rat","verb":"punch"},"line":"You punch fat giant rat for 1 damage!"}"#,
        ),
        (
            ToGateway::Character {
                session: "kasimir".into(),
                state: Box::new(CharacterState {
                    name: Some("Kasimir".into()),
                    race: Some("Human".into()),
                    class: Some("Warrior".into()),
                    level: Some(3),
                    hp: Some(30),
                    hp_max: Some(35),
                    exp: Some(1520),
                    stats: Some(Stats {
                        strength: Some(60),
                        agility: Some(50),
                        ..Stats::default()
                    }),
                    room_title: Some("Newhaven, Village Entrance".into()),
                    room_exits: Some(vec!["north".into(), "south".into()]),
                    in_combat: Some(false),
                    statuses: Some(vec![]),
                    prompt_form: Some("hp".into()),
                    parse: Some(ParseCounts {
                        lines: 412,
                        unclassified: 3,
                    }),
                    synced_at: Some("2026-10-19T12:00:05Z".into()),
                    updated_seq: Some(43),
                    ..CharacterState::default()
                }),
            },
            r#"{"type":"character","session":"kasimir","state":{"name":"Kasimir","race":"Human","class":"Warrior","level":3,"hp":30,"hp_max":35,"exp":1520,"stats":{"strength":60,"agility":50},"room_title":"Newhaven, Village Entrance","room_exits":["north","south"],"in_combat":false,"statuses":[],"prompt_form":"hp","parse":{"lines":412,"unclassified":3},"synced_at":"2026-10-19T12:00:05Z","updated_seq":43}}"#,
        ),
        (
            ToGateway::CharacterPatch {
                session: "kasimir".into(),
                patch: serde_json::json!({"hp": 23, "in_combat": true, "room_title": null}),
            },
            r#"{"type":"character_patch","session":"kasimir","patch":{"hp":23,"in_combat":true,"room_title":null}}"#,
        ),
        (
            ToGateway::BrainInfo {
                build: "0.1.0 (3f2a9c1)".into(),
                started_at: "2026-10-19T12:00:00Z".into(),
                world_pack: Some(WorldPackInfo {
                    path: "worlds/stock-1.11p.sqlite".into(),
                    hash: "sha256:5d41402abc4b2a76".into(),
                }),
            },
            r#"{"type":"brain_info","build":"0.1.0 (3f2a9c1)","started_at":"2026-10-19T12:00:00Z","world_pack":{"path":"worlds/stock-1.11p.sqlite","hash":"sha256:5d41402abc4b2a76"}}"#,
        ),
        (
            ToGateway::Notice {
                session: Some("kasimir".into()),
                text: "re-synced".into(),
            },
            r#"{"type":"notice","session":"kasimir","text":"re-synced"}"#,
        ),
        (
            ToGateway::Warning {
                session: None,
                text: "world pack not found; walking blind".into(),
            },
            r#"{"type":"warning","text":"world pack not found; walking blind"}"#,
        ),
    ]
}

/// The control messages, in `ToGateway` order.
fn control_messages() -> Vec<(ToGateway, &'static str)> {
    vec![
        (
            ToGateway::Resync {
                session: "kasimir".into(),
            },
            r#"{"type":"resync","session":"kasimir"}"#,
        ),
        (
            ToGateway::StateRequest {
                session: Some("kasimir".into()),
            },
            r#"{"type":"state_request","session":"kasimir"}"#,
        ),
    ]
}

fn relayed(messages: Vec<(ToGateway, &'static str)>) -> Vec<(ToClient, &'static str)> {
    messages
        .into_iter()
        .map(|(msg, json)| (msg.into_relayed().unwrap(), json))
        .collect()
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
            ToClient::Lagged {
                session: "kasimir".into(),
                missed: 120,
            },
            r#"{"type":"lagged","session":"kasimir","missed":120}"#,
        ),
        (
            ToClient::Error {
                message: "unknown session \"nobody\"".into(),
            },
            r#"{"type":"error","message":"unknown session \"nobody\""}"#,
        ),
    ]
    .into_iter()
    .chain(relayed(brain_messages()))
    .chain(relayed(control_messages()))
    .chain([
        (
            ToClient::Sent {
                session: "kasimir".into(),
                seq: 42,
                text: "********".into(),
            },
            r#"{"type":"sent","session":"kasimir","seq":42,"text":"********"}"#,
        ),
        (
            ToClient::BrainState { attached: true },
            r#"{"type":"brain_state","attached":true}"#,
        ),
        (
            ToClient::BrainLagged { missed: 17 },
            r#"{"type":"brain_lagged","missed":17}"#,
        ),
    ])
    .collect()
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
    .into_iter()
    .chain(control_messages())
    .chain(brain_messages())
    .collect()
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
        (
            ToGateway::StateRequest { session: None },
            r#"{"type":"state_request"}"#,
        ),
        (
            ToGateway::BrainInfo {
                build: "0.1.0".into(),
                started_at: "2026-10-19T12:00:00Z".into(),
                world_pack: None,
            },
            r#"{"type":"brain_info","build":"0.1.0","started_at":"2026-10-19T12:00:00Z"}"#,
        ),
        (
            ToGateway::Character {
                session: "kasimir".into(),
                state: Box::default(),
            },
            r#"{"type":"character","session":"kasimir","state":{}}"#,
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

#[test]
fn families_say_who_may_send_what() {
    for (msg, _) in to_gateway() {
        let family = msg.family();
        let want = match msg.type_name() {
            "resync" | "state_request" => Family::Control,
            "event" | "character" | "character_patch" | "brain_info" | "notice" | "warning" => {
                Family::Brain
            }
            _ => Family::Gateway,
        };
        assert_eq!(family, want, "{}", msg.type_name());
        // Relayed messages keep their type; the gateway's own are not relayed.
        let name = msg.type_name();
        match msg.into_relayed() {
            Some(relayed) => {
                assert_ne!(family, Family::Gateway);
                assert_eq!(relayed.type_name(), name);
            }
            None => assert_eq!(family, Family::Gateway, "{name}"),
        }
    }
    assert!(Family::Gateway.allowed_from(ClientKind::Ui));
    assert!(Family::Gateway.allowed_from(ClientKind::Brain));
    assert!(Family::Control.allowed_from(ClientKind::Ui));
    assert!(!Family::Control.allowed_from(ClientKind::Brain));
    assert!(Family::Brain.allowed_from(ClientKind::Brain));
    assert!(!Family::Brain.allowed_from(ClientKind::Ui));
}

#[test]
fn a_message_names_its_session_when_it_has_one() {
    for (msg, json) in to_client() {
        let value: serde_json::Value = serde_json::from_str(json).unwrap();
        assert_eq!(
            msg.session(),
            value.get("session").and_then(|s| s.as_str()),
            "{json}"
        );
    }
}

#[test]
fn a_patch_and_fields_pass_through_as_any_json() {
    let json = r#"{"type":"character_patch","session":"kasimir","patch":[1,"two",{"three":null}]}"#;
    let msg = ToGateway::from_json(json).unwrap();
    assert_eq!(msg.to_json(), json);
    // A snapshot with fields the struct doesn't know yet still reads.
    let json = r#"{"type":"character","session":"kasimir","state":{"hp":3,"mood":"grim"}}"#;
    let ToClient::Character { state, .. } = ToClient::from_json(json).unwrap() else {
        panic!("expected a character");
    };
    assert_eq!(state.hp, Some(3));
}
