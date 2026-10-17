//! The pack's tables (schema version [`SCHEMA_VERSION`]).
//!
//! Typed columns are the few fields later cards need now; every game table also keeps the
//! export's full record, as written, in `raw` (JSON). Run-time fields (`dirty`, `lastkill`,
//! monsters in the room, shop quantities, ...) are only in `raw`. Tables with large rows keep
//! their rowid: `WITHOUT ROWID` stores big rows badly (the pack nearly triples in size).

/// Bumped when a table or column changes meaning.
pub const SCHEMA_VERSION: i64 = 1;

pub const SQL: &str = r#"
CREATE TABLE pack_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);

CREATE TABLE rooms (
    map INTEGER NOT NULL,
    room INTEGER NOT NULL,
    name TEXT NOT NULL,
    light INTEGER NOT NULL,          -- below -150: too dark to see
    type INTEGER NOT NULL,           -- 0 Normal, 1 Shop, 2 Arena, 3 Lair, ...
    shop_num INTEGER NOT NULL,
    min_level INTEGER NOT NULL,      -- spawn group: genindex window of mon_group
    max_level INTEGER NOT NULL,
    gen_monster INTEGER NOT NULL,
    max_monsters INTEGER NOT NULL,
    mon_group INTEGER NOT NULL,
    regen_delay INTEGER NOT NULL,
    spec_mon INTEGER NOT NULL,
    flags INTEGER NOT NULL,
    mm_id INTEGER NOT NULL,          -- MegaMud room id (kb/megamud-formats.md section 0)
    raw TEXT NOT NULL,
    PRIMARY KEY (map, room)
);
CREATE INDEX rooms_name ON rooms (name);
CREATE INDEX rooms_mm_id ON rooms (mm_id);

CREATE TABLE exits (
    from_map INTEGER NOT NULL,
    from_room INTEGER NOT NULL,
    dir INTEGER NOT NULL,            -- 0-9: N S E W NE NW SE SW U D
    to_map INTEGER NOT NULL,         -- Map Change: parameter 1 when non-zero (inferred)
    to_room INTEGER NOT NULL,
    type TEXT NOT NULL,              -- exit type by name ("Normal", "Door", ...)
    type_code INTEGER NOT NULL,
    par1 INTEGER NOT NULL,
    par2 INTEGER NOT NULL,
    par3 INTEGER NOT NULL,
    par4 INTEGER NOT NULL,
    PRIMARY KEY (from_map, from_room, dir)
) WITHOUT ROWID;

CREATE TABLE monsters (
    number INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    level INTEGER NOT NULL,
    exp INTEGER NOT NULL,
    max_hp INTEGER NOT NULL,
    mon_type INTEGER NOT NULL,
    gen_index INTEGER NOT NULL,
    alignment INTEGER NOT NULL,
    raw TEXT NOT NULL
);
CREATE TABLE items (
    number INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    type INTEGER NOT NULL,
    weight INTEGER NOT NULL,
    cost INTEGER NOT NULL,
    currency INTEGER NOT NULL,       -- 0 copper, 1 silver, 2 gold, 3 platinum, 4 runic
    raw TEXT NOT NULL
);
CREATE TABLE shops (
    number INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    type INTEGER NOT NULL,           -- 0 General, 1 Weapons, 2 Armour, ... 8 Training
    min_level INTEGER NOT NULL,
    max_level INTEGER NOT NULL,
    markup INTEGER NOT NULL,
    train_class INTEGER NOT NULL,
    raw TEXT NOT NULL
);
CREATE TABLE shop_stock (
    shop INTEGER NOT NULL,
    slot INTEGER NOT NULL,           -- 0-19, the record's order
    item INTEGER NOT NULL,
    max_qty INTEGER NOT NULL,        -- 0: the shop only resells what players sold it
    PRIMARY KEY (shop, slot)
) WITHOUT ROWID;
CREATE TABLE spells (
    number INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    short_name TEXT NOT NULL,
    level INTEGER NOT NULL,
    mana INTEGER NOT NULL,
    type INTEGER NOT NULL,
    target INTEGER NOT NULL,
    raw TEXT NOT NULL
);
CREATE TABLE classes (number INTEGER PRIMARY KEY, name TEXT NOT NULL, raw TEXT NOT NULL);
CREATE TABLE races (number INTEGER PRIMARY KEY, name TEXT NOT NULL, raw TEXT NOT NULL);
CREATE TABLE messages (
    number INTEGER PRIMARY KEY,
    msg1 TEXT NOT NULL,
    msg2 TEXT NOT NULL,
    msg3 TEXT NOT NULL,
    raw TEXT NOT NULL
);
CREATE TABLE texts (                 -- one row per WCCTEXT record; a block is its parts in order
    number INTEGER NOT NULL,
    part INTEGER NOT NULL,
    link INTEGER NOT NULL,
    text TEXT NOT NULL,              -- decoded (kb/game-data.md "Text blocks and lookups")
    raw TEXT NOT NULL,
    PRIMARY KEY (number, part)
);
CREATE TABLE actions (id INTEGER PRIMARY KEY, name TEXT NOT NULL, raw TEXT NOT NULL);

CREATE TABLE mm_waypoints (          -- Rooms.md, in file order
    code TEXT PRIMARY KEY,
    line INTEGER NOT NULL,
    mm_id INTEGER NOT NULL,
    flags INTEGER NOT NULL,
    is_shop INTEGER NOT NULL,
    is_bank INTEGER NOT NULL,
    is_trainer INTEGER NOT NULL,
    is_boss INTEGER NOT NULL,
    min_level INTEGER NOT NULL,
    max_level INTEGER NOT NULL,
    class INTEGER NOT NULL,
    grp TEXT NOT NULL,
    name TEXT NOT NULL
);
CREATE TABLE mm_waypoint_rooms (     -- the game rooms whose mm_id equals the waypoint's
    code TEXT NOT NULL,
    map INTEGER NOT NULL,
    room INTEGER NOT NULL,
    PRIMARY KEY (code, map, room)
) WITHOUT ROWID;

CREATE TABLE mm_paths (              -- every .mp file
    file TEXT PRIMARY KEY,
    description TEXT NOT NULL,
    author TEXT NOT NULL,
    start_location TEXT,
    end_location TEXT,
    header TEXT NOT NULL,            -- the header line as written
    from_id INTEGER NOT NULL,
    to_id INTEGER NOT NULL,
    is_loop INTEGER NOT NULL,        -- from_id = to_id
    step_count INTEGER,
    cost INTEGER,
    item TEXT NOT NULL,
    prepath TEXT NOT NULL,
    nextpath TEXT NOT NULL,
    -- verification: the best walk through the rooms
    start_candidates INTEGER NOT NULL,
    start_map INTEGER,               -- NULL: no room has the start id or its title hash
    start_room INTEGER,
    end_map INTEGER,
    end_room INTEGER,
    walked INTEGER NOT NULL,         -- 1: walked to the last step
    closes INTEGER NOT NULL,         -- 1: a loop that walked to the end in its start room
    steps_checked INTEGER NOT NULL,
    steps_exact INTEGER NOT NULL,
    steps_title INTEGER NOT NULL,    -- title hash only
    steps_none INTEGER NOT NULL,
    steps_wildcard INTEGER NOT NULL,
    broke_at INTEGER,                -- step index (0-based) that couldn't be followed
    broke_reason TEXT
);
CREATE TABLE mm_path_steps (
    file TEXT NOT NULL,
    step INTEGER NOT NULL,           -- 0-based
    mm_id INTEGER NOT NULL,          -- 4294967295 = FFFFFFFF, any room
    flags INTEGER NOT NULL,
    command TEXT NOT NULL,           -- as written, dir[actions] included
    base TEXT NOT NULL,
    actions TEXT,                    -- JSON list; NULL when there is no [..]
    walk_map INTEGER,                -- the room the walk typed it in (NULL: not reached)
    walk_room INTEGER,
    id_match TEXT,                   -- exact, title, none, wildcard
    PRIMARY KEY (file, step)
) WITHOUT ROWID;
CREATE VIEW mm_loops AS SELECT * FROM mm_paths WHERE is_loop = 1;
CREATE VIEW mm_loop_steps AS
    SELECT s.* FROM mm_path_steps s JOIN mm_paths p ON p.file = s.file WHERE p.is_loop = 1;
"#;
