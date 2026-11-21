//! Credential references inside GitHub Actions expressions.
//!
//! GitHub evaluates `${{ ... }}` spans in every workflow value and the whole
//! value of `if:` conditions. Context and property names in expressions are
//! case-insensitive, and the `secrets` and `github` contexts can be read as a
//! whole (`toJSON(secrets)`), by index (`secrets['X']`, `secrets[format(...)]`),
//! or by property. Only an exact `secrets.<NAME>` whose name is allow-listed as
//! non-release is credential-free; every other reference to the `secrets`
//! context, and any read of `github.token` or of the whole `github` context, is
//! treated as release-credential bearing.

/// Exact `secrets.<NAME>` references that never carry a release credential.
/// No workflow needs one today; add a name here only with a reviewed reason.
const NON_RELEASE_SECRET_NAMES: &[&str] = &[];

/// Returns whether `value` references a credential-bearing context. Set
/// `whole_value_is_expression` for `if:` values, which GitHub evaluates
/// without `${{ }}` delimiters.
pub(super) fn references_credential(value: &str, whole_value_is_expression: bool) -> bool {
    if whole_value_is_expression && expression_references_credential(value) {
        return true;
    }
    expression_spans(value).any(expression_references_credential)
}

fn expression_spans(value: &str) -> impl Iterator<Item = &str> {
    let mut rest = value;
    std::iter::from_fn(move || {
        let start = rest.find("${{")?;
        let after = rest.get(start + 3..)?;
        // An unterminated span is still evaluated as far as it goes.
        let (span, remainder) = after.split_once("}}").unwrap_or((after, ""));
        rest = remainder;
        Some(span)
    })
}

#[derive(Debug, PartialEq, Eq)]
enum Token<'a> {
    Identifier(&'a str),
    Punctuation(char),
}

fn tokens(expression: &str) -> Vec<Token<'_>> {
    let mut output = Vec::new();
    let mut characters = expression.char_indices().peekable();
    while let Some((start, character)) = characters.next() {
        if character == '\'' {
            // String literal; `''` escapes a quote inside it.
            while let Some((_, next)) = characters.next() {
                if next == '\'' {
                    if characters.peek().is_some_and(|(_, after)| *after == '\'') {
                        characters.next();
                    } else {
                        break;
                    }
                }
            }
        } else if is_identifier_character(character) {
            let mut end = start + character.len_utf8();
            while let Some((index, next)) = characters.peek().copied() {
                if !is_identifier_character(next) {
                    break;
                }
                end = index + next.len_utf8();
                characters.next();
            }
            if let Some(identifier) = expression.get(start..end) {
                output.push(Token::Identifier(identifier));
            }
        } else if !character.is_whitespace() {
            output.push(Token::Punctuation(character));
        }
    }
    output
}

fn is_identifier_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
}

fn expression_references_credential(expression: &str) -> bool {
    let tokens = tokens(expression);
    tokens.iter().enumerate().any(|(index, token)| {
        let Token::Identifier(name) = token else {
            return false;
        };
        // `x.secrets` is a property of another context, not the context itself.
        if index
            .checked_sub(1)
            .and_then(|previous| tokens.get(previous))
            .is_some_and(|previous| *previous == Token::Punctuation('.'))
        {
            return false;
        }
        let property = match (tokens.get(index + 1), tokens.get(index + 2)) {
            (Some(Token::Punctuation('.')), Some(Token::Identifier(property))) => Some(*property),
            _ => None,
        };
        if name.eq_ignore_ascii_case("secrets") {
            return !property.is_some_and(|property| {
                NON_RELEASE_SECRET_NAMES
                    .iter()
                    .any(|allowed| allowed.eq_ignore_ascii_case(property))
            });
        }
        if name.eq_ignore_ascii_case("github") {
            return property.is_none_or(|property| property.eq_ignore_ascii_case("token"));
        }
        false
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_secrets_context_form_is_credential_bearing() {
        for value in [
            "${{ secrets.GITHUB_TOKEN }}",
            "${{ SECRETS.other_name }}",
            "${{ toJSON(secrets) }}",
            "${{ secrets['GH_TOKEN'] }}",
            "${{ secrets[format('{0}_TOKEN', 'GH')] }}",
            "${{ secrets.* }}",
            "${{ Secrets . CARGO_REGISTRY_TOKEN }}",
            "prefix ${{ inputs.tag }} then ${{ fromJSON(toJSON(secrets)).X }}",
            "${{ secrets.GH_TOKEN",
            "${{ github.token }}",
            "${{ GitHub.Token }}",
            "${{ github['token'] }}",
            "${{ toJSON(github) }}",
        ] {
            assert!(references_credential(value, false), "{value}");
        }
    }

    #[test]
    fn conditions_are_expressions_without_delimiters() {
        assert!(references_credential("secrets.GH_TOKEN != ''", true));
        assert!(!references_credential("secrets.GH_TOKEN != ''", false));
    }

    #[test]
    fn non_credential_expressions_and_plain_text_are_accepted() {
        for value in [
            "${{ inputs.tag }}",
            "${{ github.sha }}",
            "${{ github.event.pull_request.secrets }}",
            "${{ format('secrets {0}', github.ref) }}",
            "${{ contains('it''s secrets', 'x') }}",
            "rotate the secrets regularly",
            "https://github.com/owner/repository",
        ] {
            assert!(!references_credential(value, false), "{value}");
        }
    }
}
