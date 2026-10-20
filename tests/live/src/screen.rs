//! Reading the game's screen text: ANSI removed, the editor's cursor, numbered lists.

use std::sync::LazyLock;

use regex::Regex;

static ESCAPE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\x1b\[[0-9;?]*[A-Za-z]").unwrap());
static CURSOR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\x1b\[(\d+);(\d+)[fH]").unwrap());
static ITEM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[(\d+)\]\s+([A-Za-z-]+)").unwrap());
static ERASED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^\x08]\x08").unwrap());

/// `text` without its ANSI escapes.
pub fn plain(text: &str) -> String {
    ESCAPE.replace_all(text, "").into_owned()
}

/// `text` as a terminal shows it: no ANSI escapes, and backspaces applied (the game's direction
/// names carry a letter and a backspace, `sN\x08outh`).
pub fn shown(text: &str) -> String {
    let mut s = plain(text);
    while ERASED.is_match(&s) {
        s = ERASED.replace_all(&s, "").into_owned();
    }
    s
}

/// The last cursor position set in `text` (`ESC[row;colf`), which in the character editor
/// is the field being edited (kb/connection.md, "Driving the creation editor over ANSI").
pub fn last_cursor(text: &str) -> Option<(u16, u16)> {
    CURSOR
        .captures_iter(text)
        .last()
        .and_then(|c| Some((c[1].parse().ok()?, c[2].parse().ok()?)))
}

/// The number of `name` in a numbered list such as the race list (`[1]  Human`), ignoring
/// case.
pub fn list_number(text: &str, name: &str) -> Option<u32> {
    ITEM.captures_iter(&plain(text))
        .find(|c| c[2].eq_ignore_ascii_case(name))
        .and_then(|c| c[1].parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_are_removed() {
        assert_eq!(plain("\x1b[1;37mUsername:\x1b[0m "), "Username: ");
    }

    #[test]
    fn backspaces_are_applied() {
        assert_eq!(shown("to the sN\x08outh."), "to the south.");
        assert_eq!(shown("ab\x08\x08c"), "c");
    }

    #[test]
    fn the_last_cursor_position_wins() {
        let text = "\x1b[23;1f\x1b[1;31m   \x1b[9;34f50\x1b[20;33f 80\x1b[10;34f";
        assert_eq!(last_cursor(text), Some((10, 34)));
        assert_eq!(last_cursor("no cursor"), None);
    }

    #[test]
    fn numbered_lists_give_the_number_of_a_name() {
        let text = "\x1b[36m[1]  Human\r\n[2]  Dwarf\r\n[10] Half-Ogre\r\n";
        assert_eq!(list_number(text, "human"), Some(1));
        assert_eq!(list_number(text, "Half-Ogre"), Some(10));
        assert_eq!(list_number(text, "Elf"), None);
    }
}
