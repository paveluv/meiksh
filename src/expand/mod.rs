mod arithmetic;
mod core;
mod glob;
mod model;
mod parameter;
mod pathname;
mod word;

pub use core::{Context, ExpandError};
pub use glob::pattern_matches;
pub use word::{
    expand_assignment_value, expand_here_document, expand_parameter_text, expand_redirect_word,
    expand_word, expand_word_as_declaration_assignment, expand_word_pattern, expand_word_text,
    expand_words, word_is_assignment,
};

#[cfg(test)]
pub(super) mod test_support;
