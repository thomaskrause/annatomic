#[cfg(test)]
pub(crate) mod example_generator;
pub(crate) mod token_helper;

pub(crate) fn make_whitespace_visible<S: AsRef<str>>(v: S) -> String {
    let result: String = v
        .as_ref()
        .chars()
        .map(|c| match c {
            ' ' => '␣',
            '\n' => '↵',
            _ => c,
        })
        .collect();
    result
}
