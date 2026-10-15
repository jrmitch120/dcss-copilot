//! The app's log: one line per problem it carries on from, on stderr. Lines are made from errors
//! and names, never from a key, which can't be printed unmasked anyway.

/// Logs `line`.
pub(crate) fn error(line: impl std::fmt::Display) {
    let line = line.to_string();
    #[cfg(test)]
    tests::LOGGED.with_borrow_mut(|logged| logged.push(line.clone()));
    eprintln!("{line}");
}

#[cfg(test)]
pub(crate) mod tests {
    use std::cell::RefCell;

    thread_local! {
        /// What this test's thread has logged.
        pub(super) static LOGGED: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    }

    /// Every line this test's thread has logged so far.
    pub(crate) fn logged() -> Vec<String> {
        LOGGED.with_borrow(Clone::clone)
    }
}
