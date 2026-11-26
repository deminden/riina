//! Parse profile commands once and keep native prefixes distinct from the command.
use super::InputActionError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputCommand {
    tokens: Vec<String>,
    prefix_len: usize,
}

impl InputCommand {
    pub fn arguments(&self) -> &[String] {
        &self.tokens[self.prefix_len..]
    }

    pub fn prefixes(&self) -> &[String] {
        &self.tokens[..self.prefix_len]
    }

    pub fn repeat_override(&self) -> Option<bool> {
        self.prefixes()
            .iter()
            .rev()
            .find_map(|flag| match flag.as_str() {
                "repeatable" => Some(true),
                "nonrepeatable" => Some(false),
                _ => None,
            })
    }

    pub fn into_native(self) -> Vec<String> {
        let mut native = Vec::with_capacity(self.tokens.len() + 2);
        if !self.prefixes().iter().any(|flag| is_osd_prefix(flag)) {
            native.push("osd-auto".into());
        }
        // The string form used by input.conf expands properties by default.
        // An argument list passed to libmpv needs that behavior requested.
        if !self
            .prefixes()
            .iter()
            .any(|flag| matches!(flag.as_str(), "raw" | "expand-properties"))
            && self
                .arguments()
                .iter()
                .any(|argument| argument.contains('$'))
        {
            native.push("expand-properties".into());
        }
        native.extend(self.tokens);
        native
    }
}

fn is_osd_prefix(flag: &str) -> bool {
    matches!(
        flag,
        "osd-auto" | "no-osd" | "osd-bar" | "osd-msg" | "osd-msg-bar"
    )
}

fn is_native_prefix(flag: &str) -> bool {
    is_osd_prefix(flag)
        || matches!(
            flag,
            "raw"
                | "expand-properties"
                | "repeatable"
                | "nonrepeatable"
                | "nonscalable"
                | "async"
                | "sync"
        )
}

pub fn mpv_command_prefix_len(tokens: &[String]) -> usize {
    tokens
        .iter()
        .take_while(|token| is_native_prefix(token))
        .count()
}

pub fn mpv_command_suppresses_osd(prefixes: &[String]) -> bool {
    prefixes
        .iter()
        .rev()
        .find(|flag| is_osd_prefix(flag))
        .is_some_and(|flag| flag == "no-osd")
}

pub fn mpv_action_native_args(action: &str) -> Result<Vec<String>, InputActionError> {
    parse_command(action).map(InputCommand::into_native)
}

pub(crate) fn mpv_action_native_commands(
    action: &str,
) -> Result<Vec<Vec<String>>, InputActionError> {
    Ok(parse_mpv_input(action)?
        .into_iter()
        .map(InputCommand::into_native)
        .collect())
}

pub fn mpv_action_commands(action: &str) -> Result<Vec<Vec<String>>, InputActionError> {
    Ok(parse_mpv_input(action)?
        .into_iter()
        .map(|mut command| command.tokens.split_off(command.prefix_len))
        .collect())
}

fn parse_command(action: &str) -> Result<InputCommand, InputActionError> {
    if action.is_empty() || action.len() > 8192 || action.contains('\0') {
        return Err(InputActionError::InvalidAction);
    }

    let mut args = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut escaped = false;
    let mut token_started = false;
    for character in action.chars() {
        if escaped {
            current.push(character);
            escaped = false;
            token_started = true;
            continue;
        }
        if character == '\\' {
            escaped = true;
            token_started = true;
            continue;
        }
        if let Some(delimiter) = quote {
            if character == delimiter {
                quote = None;
            } else {
                current.push(character);
            }
            token_started = true;
            continue;
        }
        match character {
            '"' | '\'' => {
                quote = Some(character);
                token_started = true;
            }
            value if value.is_whitespace() => {
                if token_started {
                    args.push(std::mem::take(&mut current));
                    token_started = false;
                    if args.len() > 64 {
                        return Err(InputActionError::TooManyArguments);
                    }
                }
            }
            value => {
                current.push(value);
                token_started = true;
            }
        }
        if current.len() > 4096 {
            return Err(InputActionError::ArgumentTooLarge);
        }
    }
    if escaped || quote.is_some() {
        return Err(InputActionError::UnfinishedEscapeOrQuote);
    }
    if token_started {
        args.push(current);
    }
    if args.len() > 64 {
        return Err(InputActionError::TooManyArguments);
    }
    if args.iter().any(|argument| argument.len() > 4096) {
        return Err(InputActionError::ArgumentTooLarge);
    }

    let mut prefix_len = 0;
    while let Some(token) = args.get(prefix_len) {
        if token == "{default}" {
            args.remove(prefix_len);
        } else if token.starts_with('{') && token.ends_with('}') {
            return Err(InputActionError::UnsupportedInputSection);
        } else if is_native_prefix(token) {
            prefix_len += 1;
        } else {
            break;
        }
    }
    let command = args.get(prefix_len).ok_or(InputActionError::NoMpvCommand)?;
    if command.is_empty() {
        return Err(InputActionError::NoMpvCommand);
    }
    if matches!(command.as_str(), "run" | "subprocess") {
        return Err(InputActionError::ExternalProcess);
    }
    Ok(InputCommand {
        tokens: args,
        prefix_len,
    })
}

/// Convert an input mapping that chains commands with semicolons into separate
/// libmpv command argument lists. A semicolon inside a quoted or escaped token
/// remains data, matching input.conf's command separator behavior.
pub fn parse_mpv_input(action: &str) -> Result<Vec<InputCommand>, InputActionError> {
    if action.is_empty() || action.len() > 8192 || action.contains('\0') {
        return Err(InputActionError::InvalidAction);
    }

    let mut commands = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut escaped = false;
    for character in action.chars() {
        if escaped {
            current.push(character);
            escaped = false;
            continue;
        }
        if character == '\\' {
            current.push(character);
            escaped = true;
            continue;
        }
        if let Some(delimiter) = quote {
            current.push(character);
            if character == delimiter {
                quote = None;
            }
            continue;
        }
        match character {
            '"' | '\'' => {
                quote = Some(character);
                current.push(character);
            }
            ';' => {
                push_mpv_action_command(&mut commands, &mut current)?;
            }
            _ => current.push(character),
        }
    }
    if escaped || quote.is_some() {
        return Err(InputActionError::UnfinishedEscapeOrQuote);
    }
    push_mpv_action_command(&mut commands, &mut current)?;
    if commands.is_empty() {
        return Err(InputActionError::NoMpvCommand);
    }
    Ok(commands)
}

fn push_mpv_action_command(
    commands: &mut Vec<InputCommand>,
    current: &mut String,
) -> Result<(), InputActionError> {
    if current.trim().is_empty() {
        return Err(InputActionError::EmptyCommand);
    }
    if commands.len() >= 32 {
        return Err(InputActionError::TooManyCommands);
    }
    commands.push(parse_command(current.trim())?);
    current.clear();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefixes_survive_native_submission_and_keep_input_validation() {
        let command = parse_mpv_input(
            "{default} repeatable nonrepeatable osd-msg no-osd seek 3 relative+exact",
        )
        .unwrap()
        .pop()
        .unwrap();
        assert_eq!(command.repeat_override(), Some(false));
        assert!(mpv_command_suppresses_osd(command.prefixes()));
        assert_eq!(command.arguments(), ["seek", "3", "relative+exact"]);
        assert_eq!(
            command.into_native(),
            [
                "repeatable",
                "nonrepeatable",
                "osd-msg",
                "no-osd",
                "seek",
                "3",
                "relative+exact"
            ]
        );
        assert_eq!(
            mpv_action_native_args("show-text '${filename}; Ready'").unwrap(),
            [
                "osd-auto",
                "expand-properties",
                "show-text",
                "${filename}; Ready"
            ]
        );
        assert_eq!(
            mpv_action_native_args("raw show-text '${filename}'").unwrap(),
            ["osd-auto", "raw", "show-text", "${filename}"]
        );
        assert_eq!(
            parse_mpv_input("no-osd seek 1; async subprocess forbidden"),
            Err(InputActionError::ExternalProcess)
        );
        assert_eq!(
            parse_mpv_input("{encode} seek 1"),
            Err(InputActionError::UnsupportedInputSection)
        );
    }
}
