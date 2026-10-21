//! Custom loops: loops the player made from an existing one by adding, removing or reordering
//! stops, kept in the store and run like any other (docs/brain.md "Custom loops"). This module
//! holds the pure parts: naming and checking.

/// The id prefix of a custom loop.
pub const PREFIX: &str = "custom:";
/// The longest name, characters.
pub const NAME_MAX: usize = 60;
/// The most stops a custom loop may have.
pub const STOPS_MAX: usize = 300;

/// A name as a slug: lower-case letters and digits, runs of anything else becoming one `-`,
/// none at either end. `None` when nothing is left ("!!!").
pub fn slug(name: &str) -> Option<String> {
    let mut out = String::new();
    let mut gap = false;
    for c in name.chars() {
        if c.is_alphanumeric() {
            if gap && !out.is_empty() {
                out.push('-');
            }
            gap = false;
            out.extend(c.to_lowercase());
        } else {
            gap = true;
        }
    }
    (!out.is_empty()).then_some(out)
}

/// What a name is made into: the trimmed name and the loop's id, or why not.
pub fn name_and_id(name: &str) -> Result<(String, String), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("a loop needs a name".to_owned());
    }
    if name.chars().count() > NAME_MAX {
        return Err(format!("a loop's name is at most {NAME_MAX} characters"));
    }
    let slug = slug(name).ok_or_else(|| "a loop's name needs a letter or a digit".to_owned())?;
    Ok((name.to_owned(), format!("{PREFIX}{slug}")))
}

/// Whether `id` names a custom loop.
pub fn is_custom(id: &str) -> bool {
    id.len() > PREFIX.len() && id[..PREFIX.len()].eq_ignore_ascii_case(PREFIX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_becomes_a_slug() {
        assert_eq!(
            slug("Hidden Keep with Leo the Quick").as_deref(),
            Some("hidden-keep-with-leo-the-quick")
        );
        assert_eq!(
            slug("  Bandit  Keep!! (v2) ").as_deref(),
            Some("bandit-keep-v2")
        );
        assert_eq!(slug("Ünïcode Loop").as_deref(), Some("ünïcode-loop"));
        assert_eq!(slug("!!!"), None);
        assert_eq!(slug(""), None);
    }

    #[test]
    fn names_are_checked_and_ids_are_prefixed() {
        assert_eq!(
            name_and_id("  Hidden Keep with Leo the Quick ").unwrap(),
            (
                "Hidden Keep with Leo the Quick".to_owned(),
                "custom:hidden-keep-with-leo-the-quick".to_owned()
            )
        );
        assert!(name_and_id("   ").is_err());
        assert!(name_and_id("???").is_err());
        assert!(name_and_id(&"x".repeat(NAME_MAX + 1)).is_err());
        assert!(name_and_id(&"x".repeat(NAME_MAX)).is_ok());
        // The same name in another case is the same loop.
        assert_eq!(name_and_id("LEO").unwrap().1, name_and_id("leo").unwrap().1);
    }

    #[test]
    fn custom_ids_are_told_from_the_others() {
        assert!(is_custom("custom:leo"));
        assert!(is_custom("Custom:Leo"));
        assert!(!is_custom("custom:"));
        assert!(!is_custom("HKEPLOOP.mp"));
        assert!(!is_custom("1:2140:8:doors:0"));
    }
}
