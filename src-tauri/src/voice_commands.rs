//! Spoken dictation commands, applied to a finished dictation before optional
//! post-processing (opt-in: `voice_commands_enabled`, and `voice_emoji_enabled`
//! for "<name> emoji").
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

// ---------------------------------------------------------------------------
// Spoken emoji: "<name> emoji", e.g. "red heart emoji", "thumbs up emoji
// medium skin tone". Names are CLDR short names and GitHub shortcodes from the
// `emojis` crate (full Unicode set, incl. skin-tone variants), plus a few
// colloquial aliases. The trailing word "emoji" is required.

const EMOJI_WORD: &str = "emoji";

/// Colloquial names CLDR/GitHub lack. Checked after CLDR names and shortcodes.
const EMOJI_ALIASES: &[(&str, &str)] = &[
    ("crying laughing", "😂"),
    ("laughing crying", "😂"),
    ("cry laughing", "😂"),
    ("laugh crying", "😂"),
    ("tears of joy", "😂"),
    ("love", "❤️"),
    ("love heart", "❤️"),
    ("smiley face", "😃"),
    ("happy face", "😀"),
    ("sad face", "😢"),
    ("crying", "😭"),
    ("shrug", "🤷"),
    ("praying hands", "🙏"),
    ("prayer hands", "🙏"),
    ("high five", "🙏"),
    ("peace sign", "✌️"),
    ("hundred", "💯"),
    ("one hundred", "💯"),
    ("party", "🥳"),
    ("clapping", "👏"),
    ("kiss face", "😘"),
    ("blowing a kiss", "😘"),
    ("mind blown", "🤯"),
];

const SKIN_TONES: &[(&str, emojis::SkinTone)] = &[
    ("medium light", emojis::SkinTone::MediumLight),
    ("medium dark", emojis::SkinTone::MediumDark),
    ("light", emojis::SkinTone::Light),
    ("medium", emojis::SkinTone::Medium),
    ("dark", emojis::SkinTone::Dark),
];

/// Lowercase, `-`/`_`/`:`/`,` as spaces, apostrophes and quotes dropped,
/// whitespace collapsed.
fn emoji_key(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .filter(|c| !matches!(c, '\'' | '\u{2019}' | '"' | '\u{201c}' | '\u{201d}' | '.'))
        .map(|c| {
            if matches!(c, '-' | '_' | ':' | ',') {
                ' '
            } else {
                c
            }
        })
        .collect();
    cleaned
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

struct EmojiIndex {
    by_name: std::collections::HashMap<String, &'static str>,
    max_words: usize,
}

fn emoji_index() -> &'static EmojiIndex {
    static INDEX: std::sync::OnceLock<EmojiIndex> = std::sync::OnceLock::new();
    INDEX.get_or_init(|| {
        let mut by_name = std::collections::HashMap::new();
        let all = || {
            emojis::iter().flat_map(|e| {
                let tones: Vec<&'static emojis::Emoji> =
                    e.skin_tones().map(|t| t.collect()).unwrap_or_default();
                std::iter::once(e).chain(tones.into_iter().skip(1))
            })
        };
        // CLDR names take priority over shortcodes, shortcodes over aliases.
        for e in all() {
            by_name.entry(emoji_key(e.name())).or_insert(e.as_str());
            // "flag: Japan" is also "Japan flag".
            if let Some(country) = e.name().strip_prefix("flag: ") {
                by_name
                    .entry(emoji_key(&format!("{country} flag")))
                    .or_insert(e.as_str());
            }
        }
        for e in all() {
            for code in e.shortcodes() {
                by_name.entry(emoji_key(code)).or_insert(e.as_str());
            }
        }
        for (alias, emoji) in EMOJI_ALIASES {
            by_name.entry(emoji_key(alias)).or_insert(emoji);
        }
        let max_words = by_name
            .keys()
            .map(|k| k.split(' ').count())
            .max()
            .unwrap_or(1);
        EmojiIndex { by_name, max_words }
    })
}

/// Words `words[i..i + n]` as one emoji key.
fn joined_key(words: &[String], i: usize, n: usize) -> Option<String> {
    (i + n <= words.len()).then(|| emoji_key(&words[i..i + n].join(" ")))
}

/// Optional "[with] <tone> skin [tone]" at `j`; returns the tone and words used.
fn match_skin_tone(words: &[String], j: usize) -> Option<(emojis::SkinTone, usize)> {
    for n in (1..=5).rev() {
        let Some(key) = joined_key(words, j, n) else {
            continue;
        };
        let key = key.strip_prefix("with ").unwrap_or(&key);
        for (tone_name, tone) in SKIN_TONES {
            for suffix in [" skin tone", " skin"] {
                if key == format!("{tone_name}{suffix}") {
                    return Some((*tone, n));
                }
            }
        }
    }
    None
}

/// `<name words> emoji [skin tone]` starting at `i`: the emoji and words used.
fn match_emoji(words: &[String], i: usize) -> Option<(String, usize)> {
    let index = emoji_index();
    for n in (1..=index.max_words).rev() {
        if words.get(i + n).map(String::as_str) != Some(EMOJI_WORD) {
            continue;
        }
        let Some(emoji) = joined_key(words, i, n).and_then(|k| index.by_name.get(&k).copied())
        else {
            continue;
        };
        let mut used = n + 1;
        let mut out = emoji.to_string();
        if let Some((tone, tone_words)) = match_skin_tone(words, i + used) {
            if let Some(toned) = emojis::get(emoji).and_then(|e| e.with_skin_tone(tone)) {
                out = toned.as_str().to_string();
                used += tone_words;
            }
        }
        return Some((out, used));
    }
    None
}

/// `commands`: spoken punctuation/breaks/edits (`voice_commands_enabled`);
/// `emoji`: "<name> emoji" (`voice_emoji_enabled`).
pub fn apply_voice_commands(text: &str, commands: bool, emoji: bool) -> VoiceCommandOutput {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let words: Vec<String> = tokens.iter().map(|t| normalize(t)).collect();

    let mut out = String::with_capacity(text.len());
    let mut matched = false;
    let mut submit = false;
    let mut capitalize = false;
    let mut i = 0;

    while i < tokens.len() {
        if let Some((emoji_str, len)) = emoji.then(|| match_emoji(&words, i)).flatten() {
            matched = true;
            // Keep sentence punctuation the model put after the spoken name.
            let last = tokens[i + len - 1];
            let trailing = &last[last.trim_end_matches(PUNCTUATION).len()..];
            if !out.is_empty() && !out.ends_with('\n') {
                out.push(' ');
            }
            out.push_str(&emoji_str);
            out.push_str(trailing);
            capitalize = trailing.ends_with(SENTENCE_END);
            i += len;
            continue;
        }
        if let Some((command, len)) = commands.then(|| match_at(&words, i)).flatten() {
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
        apply_voice_commands(text, true, true).text
    }

    #[test]
    fn no_command_is_untouched() {
        let text = "  Hello there,  world.  ";
        assert_eq!(run(text), text);
        assert!(!apply_voice_commands(text, true, true).submit);
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
        let out = apply_voice_commands("Ship it. Press enter.", true, true);
        assert_eq!(
            out,
            VoiceCommandOutput {
                text: "Ship it.".into(),
                submit: true
            }
        );

        let out = apply_voice_commands("press enter to continue", true, true);
        assert_eq!(out.text, "press enter to continue");
        assert!(!out.submit);

        let out = apply_voice_commands("Press enter.", true, true);
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

    #[test]
    fn spoken_emoji() {
        assert_eq!(run("I love you heart emoji"), "I love you ❤️");
        assert_eq!(run("nice thumbs up emoji"), "nice 👍");
        assert_eq!(run("that was crying laughing emoji"), "that was 😂");
        assert_eq!(run("Fire emoji."), "🔥.");
        assert_eq!(run("so good fire emoji fire emoji"), "so good 🔥 🔥");
        // CLDR names, multi-word, any case, model punctuation ignored.
        assert_eq!(run("Face with tears of joy, emoji"), "😂");
        assert_eq!(run("red heart emoji period thanks"), "❤️. Thanks");
        assert_eq!(run("smiling face with heart eyes emoji"), "😍");
        assert_eq!(run("party popper emoji"), "🎉");
        assert_eq!(run("Japan flag emoji"), "🇯🇵");
    }

    #[test]
    fn spoken_emoji_skin_tones() {
        assert_eq!(run("thumbs up emoji medium skin tone"), "👍🏽");
        assert_eq!(run("waving hand emoji with dark skin tone"), "👋🏿");
        assert_eq!(run("thumbs up medium-light skin tone emoji"), "👍🏼");
        // No skin-tone variant: the tone words stay text.
        assert_eq!(run("fire emoji dark skin tone"), "🔥 dark skin tone");
    }

    #[test]
    fn emoji_requires_trailing_word() {
        for text in [
            "my heart is on fire",
            "thumbs up from me",
            "the emoji was cute",
            "unknown thing emoji",
        ] {
            assert_eq!(run(text), text);
        }
    }

    #[test]
    fn emoji_index_covers_full_set() {
        let index = emoji_index();
        assert!(index.by_name.len() > 3000, "{}", index.by_name.len());
        assert_eq!(index.by_name.get("thumbs up medium skin tone"), Some(&"👍🏽"));
    }

    #[test]
    fn features_toggle_independently() {
        let text = "hi comma heart emoji";
        assert_eq!(
            apply_voice_commands(text, true, false).text,
            "hi, heart emoji"
        );
        assert_eq!(apply_voice_commands(text, false, true).text, "hi comma ❤️");
        assert_eq!(apply_voice_commands(text, false, false).text, text);
    }
}
