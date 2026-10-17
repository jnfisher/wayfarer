//! Claiming a test character: try the pool in order and keep the first that logs on. The
//! server refuses a second logon to an account in use, so two checks running at once never
//! share a character.

/// How one try went.
#[derive(Debug)]
pub enum Attempt<T> {
    /// Logged on: the character is ours until we log it off.
    Claimed(T),
    /// Someone else is logged on with it.
    InUse,
    /// It can't be used (bad credentials, an account we have no password for, ...).
    Unusable(String),
    /// Nothing can be (the gateway won't start, say): stop trying.
    Abort(String),
}

/// Try each of `pool` in order with `attempt`, stopping at the first claimed. On success the
/// index, the claim and why each before it was passed over; otherwise why each one was.
pub async fn claim<C, T>(
    pool: &[C],
    mut attempt: impl AsyncFnMut(usize, &C) -> Attempt<T>,
) -> Result<(usize, T, Vec<String>), Vec<String>> {
    let mut passed = Vec::new();
    for (i, c) in pool.iter().enumerate() {
        match attempt(i, c).await {
            Attempt::Claimed(t) => return Ok((i, t, passed)),
            Attempt::InUse => passed.push(format!("#{}: in use", i + 1)),
            Attempt::Unusable(why) => passed.push(format!("#{}: {why}", i + 1)),
            Attempt::Abort(why) => {
                passed.push(format!("#{}: {why}", i + 1));
                break;
            }
        }
    }
    Err(passed)
}

#[cfg(test)]
mod tests {
    use super::*;

    type Claimed = Result<(usize, &'static str, Vec<String>), Vec<String>>;

    /// Claim from a pool whose tries go as `outcomes` say; the result and the indexes tried.
    fn run(outcomes: &[&str]) -> (Claimed, Vec<usize>) {
        let mut tried = Vec::new();
        let result = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(claim(outcomes, async |i, o: &&str| {
                tried.push(i);
                match *o {
                    "free" => Attempt::Claimed("ours"),
                    "busy" => Attempt::InUse,
                    "broken" => Attempt::Abort("no gateway".to_owned()),
                    why => Attempt::Unusable(why.to_owned()),
                }
            }));
        (result, tried)
    }

    #[test]
    fn the_first_free_character_is_claimed_and_none_after_it_is_tried() {
        let (result, tried) = run(&["free", "free"]);
        assert_eq!(result.unwrap(), (0, "ours", vec![]));
        assert_eq!(tried, [0]);
    }

    #[test]
    fn characters_in_use_or_unusable_are_passed_over_in_order() {
        let (result, tried) = run(&["busy", "bad credentials", "free", "free"]);
        let passed = vec!["#1: in use".to_owned(), "#2: bad credentials".to_owned()];
        assert_eq!(result.unwrap(), (2, "ours", passed));
        assert_eq!(tried, [0, 1, 2]);
    }

    #[test]
    fn an_abort_stops_the_search() {
        let (result, tried) = run(&["busy", "broken", "free"]);
        assert_eq!(result.unwrap_err(), ["#1: in use", "#2: no gateway"]);
        assert_eq!(tried, [0, 1]);
    }

    #[test]
    fn an_exhausted_pool_says_why_each_was_passed_over() {
        let (result, tried) = run(&["busy", "bad credentials"]);
        assert_eq!(result.unwrap_err(), ["#1: in use", "#2: bad credentials"]);
        assert_eq!(tried, [0, 1]);
    }
}
