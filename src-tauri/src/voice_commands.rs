//! Spoken dictation commands, applied to a finished dictation before optional
//! post-processing (opt-in: `voice_commands_enabled`).
//!
//! Matching is whole-word, case-insensitive and ignores punctuation the model
//! attached to the spoken words ("Comma," matches `comma`). A transcript with no
//! command is returned untouched.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    /// Attach punctuation to the previous word.
    Punct(&'static str),
    /// Insert a line break; the next word is capitalized.
    Break(&'static str),
    /// Delete the previous sentence (back to the last `.`, `!`, `?` or newline).
    ScratchThat,
    /// Press the auto-submit key after pasting. Only honored as the last words.
    PressEnter,
}

/// Longest phrases first so greedy matching prefers them.
const COMMANDS: &[(&[&str], Command)] = &[
    (&["new", "paragraph"], Command::Break("\n\n")),
    (&["new", "line"], Command::Break("\n")),
    (&["question", "mark"], Command::Punct("?")),
    (&["exclamation", "mark"], Command::Punct("!")),
    (&["exclamation", "point"], Command::Punct("!")),
    (&["full", "stop"], Command::Punct(".")),
    (&["scratch", "that"], Command::ScratchThat),
    (&["press", "enter"], Command::PressEnter),
    (&["period"], Command::Punct(".")),
    (&["comma"], Command::Punct(",")),
    (&["colon"], Command::Punct(":")),
    (&["semicolon"], Command::Punct(";")),
];

const PUNCTUATION: &[char] = &['.', ',', ';', ':', '!', '?'];
const SENTENCE_END: &[char] = &['.', '!', '?'];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceCommandOutput {
    pub text: String,
    /// "press enter" ended the dictation: submit after pasting.
    pub submit: bool,
}

fn normalize(word: &str) -> String {
    word.trim_matches(|c: char| !c.is_alphanumeric())
        .to_lowercase()
}

fn match_at(words: &[String], i: usize) -> Option<(Command, usize)> {
    COMMANDS.iter().find_map(|(phrase, command)| {
        let end = i + phrase.len();
        let hit =
            end <= words.len() && words[i..end].iter().zip(phrase.iter()).all(|(w, p)| w == p);
        // "press enter" mid-sentence is dictated text, not a key press.
        let allowed = *command != Command::PressEnter || end == words.len();
        (hit && allowed).then_some((*command, phrase.len()))
    })
}

fn trim_end_spaces(text: &mut String) {
    let len = text.trim_end_matches([' ', '\t']).len();
    text.truncate(len);
}

fn scratch_last_sentence(text: &mut String) {
    let body = text.trim_end_matches(|c: char| c.is_whitespace() || PUNCTUATION.contains(&c));
    let keep = body
        .rfind(|c: char| SENTENCE_END.contains(&c) || c == '\n')
        .map_or(0, |i| i + 1);
    text.truncate(keep);
    trim_end_spaces(text);
}

fn push_capitalized(out: &mut String, word: &str) {
    let mut chars = word.chars();
    if let Some(first) = chars.next() {
        out.extend(first.to_uppercase());
        out.push_str(chars.as_str());
    }
}

pub fn apply_voice_commands(text: &str) -> VoiceCommandOutput {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let words: Vec<String> = tokens.iter().map(|t| normalize(t)).collect();

    let mut out = String::with_capacity(text.len());
    let mut matched = false;
    let mut submit = false;
    let mut capitalize = false;
    let mut i = 0;

    while i < tokens.len() {
        if let Some((command, len)) = match_at(&words, i) {
            matched = true;
            i += len;
            match command {
                Command::Punct(p) => {
                    // Replace punctuation the model guessed with the spoken one.
                    let len = out.trim_end().trim_end_matches(PUNCTUATION).len();
                    out.truncate(len);
                    out.push_str(p);
                    capitalize = p.ends_with(SENTENCE_END);
                }
                Command::Break(b) => {
                    trim_end_spaces(&mut out);
                    out.push_str(b);
                    capitalize = true;
                }
                Command::ScratchThat => {
                    scratch_last_sentence(&mut out);
                    capitalize =
                        out.is_empty() || out.ends_with(SENTENCE_END) || out.ends_with('\n');
                }
                Command::PressEnter => submit = true,
            }
            continue;
        }

        if !out.is_empty() && !out.ends_with('\n') {
            out.push(' ');
        }
        if capitalize {
            push_capitalized(&mut out, tokens[i]);
        } else {
            out.push_str(tokens[i]);
        }
        capitalize = false;
        i += 1;
    }

    if !matched {
        return VoiceCommandOutput {
            text: text.to_string(),
            submit: false,
        };
    }

    trim_end_spaces(&mut out);
    let len = out.trim_start().len();
    out.drain(..out.len() - len);
    VoiceCommandOutput { text: out, submit }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: &str) -> String {
        apply_voice_commands(text).text
    }

    #[test]
    fn no_command_is_untouched() {
        let text = "  Hello there,  world.  ";
        assert_eq!(run(text), text);
        assert!(!apply_voice_commands(text).submit);
    }

    #[test]
    fn punctuation_replaces_model_punctuation() {
        assert_eq!(
            run("hello comma how are you question mark"),
            "hello, how are you?"
        );
        assert_eq!(
            run("Hello, comma, how are you? Question mark."),
            "Hello, how are you?"
        );
        assert_eq!(run("done period next"), "done. Next");
        assert_eq!(run("wow exclamation point"), "wow!");
        assert_eq!(
            run("note colon buy milk semicolon eggs"),
            "note: buy milk; eggs"
        );
    }

    #[test]
    fn line_breaks() {
        assert_eq!(
            run("first line. New line. second line"),
            "first line.\nSecond line"
        );
        assert_eq!(run("one new paragraph two"), "one\n\nTwo");
        assert_eq!(run("end new line"), "end\n");
    }

    #[test]
    fn scratch_that_drops_previous_sentence() {
        assert_eq!(
            run("Keep this. Drop this, scratch that. and this"),
            "Keep this. And this"
        );
        assert_eq!(run("drop everything scratch that"), "");
        assert_eq!(
            run("line one new line oops scratch that two"),
            "line one\nTwo"
        );
    }

    #[test]
    fn press_enter_only_at_end() {
        let out = apply_voice_commands("Ship it. Press enter.");
        assert_eq!(
            out,
            VoiceCommandOutput {
                text: "Ship it.".into(),
                submit: true
            }
        );

        let out = apply_voice_commands("press enter to continue");
        assert_eq!(out.text, "press enter to continue");
        assert!(!out.submit);

        let out = apply_voice_commands("Press enter.");
        assert_eq!(
            out,
            VoiceCommandOutput {
                text: String::new(),
                submit: true
            }
        );
    }

    #[test]
    fn whole_words_only() {
        assert_eq!(
            run("periodic commas semicolons"),
            "periodic commas semicolons"
        );
    }
}
