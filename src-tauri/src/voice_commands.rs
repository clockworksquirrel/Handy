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
// Spoken emoji: "<name> emoji", e.g. "red heart emoji", "sticking tongue out
// emoji", "thumbs up emoji medium skin tone". The trailing word "emoji" is
// required.
//
// 1. Exact: curated aliases > CLDR names > shortcodes (gemoji via `emojis`,
//    emojibase, iamcal), incl. skin-tone variants and "<country> flag".
// 2. Forgiving (only when "emoji" ends the phrase: last word, followed by
//    punctuation, a skin tone or a voice command): the spoken words are stemmed,
//    function words dropped, and every remaining word must appear in one
//    emoji's names or CLDR/emojibase/gemoji keywords (`emoji_keywords.tsv`).
//    Best Jaccard overlap with a single name (keywords weigh 0.85) wins, soft
//    words ("face", "sticking", "out", ...) only break ties, then popularity.

const EMOJI_WORD: &str = "emoji";
const EMOJI_KEYWORDS: &str = include_str!("emoji_keywords.tsv");

/// Colloquial names. Highest priority: checked before CLDR names and shortcodes.
const EMOJI_ALIASES: &[(&str, &str)] = &[
    ("crying laughing", "😂"),
    ("laughing crying", "😂"),
    ("cry laughing", "😂"),
    ("laugh crying", "😂"),
    ("laughing", "😂"),
    ("lol", "😂"),
    ("lmao", "🤣"),
    ("tears of joy", "😂"),
    ("love", "❤️"),
    ("love heart", "❤️"),
    ("smiley face", "😃"),
    ("happy face", "😀"),
    ("happy", "😊"),
    ("smiling", "😊"),
    ("big smile", "😄"),
    ("sad", "😢"),
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
    ("kiss", "😘"),
    ("kiss face", "😘"),
    ("kissy face", "😘"),
    ("blowing a kiss", "😘"),
    ("mind blown", "🤯"),
    ("eye roll", "🙄"),
    ("rolling eyes", "🙄"),
    ("tongue out", "😛"),
    ("cool", "😎"),
    ("ok", "👌"),
    ("okay", "👌"),
    ("check mark", "✅"),
    ("checkmark", "✅"),
    ("check", "✅"),
    ("flex", "💪"),
    ("money", "💰"),
    ("hundred percent", "💯"),
    ("100 percent", "💯"),
    ("thank you", "🙏"),
    ("angel", "😇"),
    ("dead", "💀"),
    ("im dead", "💀"),
    ("scared", "😨"),
    ("shocked", "😮"),
    ("surprised", "😮"),
    ("mad", "😡"),
    ("crazy face", "🤪"),
    ("side eye", "😒"),
    ("throwing up", "🤮"),
    ("lightbulb", "💡"),
    ("pin", "📌"),
    ("power wheelchair", "🧑‍🦼"),
    ("electric wheelchair", "🧑‍🦼"),
    ("manual wheelchair", "🧑‍🦽"),
    ("wheelchair user", "🧑‍🦽"),
];

const EMOJI_BY_POPULARITY: &[&str] = &[
    "😂", "❤️", "🤣", "👍", "😭", "🙏", "😘", "🥰", "😍", "😊", "🎉", "😁", "💕", "🥺", "😅", "🔥",
    "☺️", "🤦", "♥️", "🤷", "🙄", "😆", "🤗", "😉", "🎂", "🤔", "👏", "🙂", "😳", "🥳", "😎", "👌",
    "💜", "😔", "💪", "✨", "💖", "👀", "😋", "😏", "😢", "👉", "💗", "😩", "💯", "🌹", "💞", "🎈",
    "💙", "😃", "😡", "💐", "😜", "🙈", "🤞", "😄", "🤤", "🙌", "🤪", "❣️", "😀", "💋", "💀", "👇",
    "💔", "😌", "💓", "🤩", "🙃", "😬", "😱", "😴", "🤭", "😐", "🌞", "😒", "😇", "🌸", "😈", "🎶",
    "✌️", "🎊", "🥵", "😞", "💚", "☀️", "🖤", "💰", "😚", "👑", "🎁", "💥", "🙋", "☹️", "😑", "🥴",
    "👈", "💩", "✅", "👋", "🤯", "😛", "😝", "🫶", "🤡", "👻", "🎃", "🚀", "⭐", "🌈", "🍕", "☕",
    "🍺", "🍷",
];

const SKIN_TONES: &[(&str, emojis::SkinTone)] = &[
    ("medium light", emojis::SkinTone::MediumLight),
    ("medium dark", emojis::SkinTone::MediumDark),
    ("light", emojis::SkinTone::Light),
    ("medium", emojis::SkinTone::Medium),
    ("dark", emojis::SkinTone::Dark),
];

/// Dropped from spoken and indexed names; never required, never matched.
const EMOJI_STOPWORDS: &[&str] = &[
    "a", "an", "the", "my", "your", "our", "his", "her", "their", "its", "this", "that", "these",
    "those", "of", "and", "or", "with", "to", "for", "in", "on", "at", "by", "from", "is", "are",
    "was", "were", "be", "it", "i", "im", "me", "you", "we", "they", "he", "she", "him", "them",
    "us", "so", "very", "just", "really", "please", "some", "any", "but",
];

/// Pronouns and the like: a forgiving name never spans them ("I love you
/// heart emoji" -> "I love you ❤️").
const EMOJI_PHRASE_BREAKERS: &[&str] = &[
    "is", "are", "was", "were", "be", "it", "i", "im", "me", "you", "we", "they", "he", "she",
    "him", "them", "us", "so", "very", "just", "really", "please", "but", "or",
];

/// Filler that need not match, but counts as a tie-breaker when it does
/// ("sticking tongue out" prefers 😛 `stuck_out_tongue` over 👅).
const EMOJI_SOFT_WORDS: &[&str] = &["face", "sticking", "stuck", "out", "little", "big", "small"];

/// Words before "emoji" that make it a noun, not a name ("that emoji").
const EMOJI_NOUN_MARKERS: &[&str] = &[
    "a",
    "an",
    "the",
    "that",
    "this",
    "these",
    "those",
    "my",
    "your",
    "his",
    "her",
    "their",
    "our",
    "its",
    "which",
    "what",
    "any",
    "some",
    "every",
    "each",
    "no",
    "another",
    "new",
    "favorite",
    "favourite",
    "own",
    "best",
    "same",
    "other",
    "first",
    "last",
];

const EMOJI_MAX_FUZZY_WORDS: usize = 6;
const EMOJI_MIN_SCORE: f32 = 0.5;
const EMOJI_KEYWORD_WEIGHT: f32 = 0.85;
const EMOJI_SOFT_BONUS: f32 = 0.15;

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

/// Small suffix stemmer, applied identically to spoken and indexed words:
/// smiling/smile/smiled -> smil, eyes/eye -> ey, clapping -> clap, cries -> cry.
fn emoji_stem(word: &str) -> String {
    let irregular = match word {
        "stuck" => Some("stick"),
        "teeth" => Some("tooth"),
        "feet" => Some("foot"),
        "men" => Some("man"),
        "women" => Some("woman"),
        "people" => Some("person"),
        _ => None,
    };
    if let Some(stem) = irregular {
        return stem.to_string();
    }
    let mut s = word.to_string();
    if let Some(base) = s.strip_suffix("ies").or_else(|| s.strip_suffix("ied")) {
        if base.len() >= 2 {
            s = format!("{base}y");
        }
    } else if let Some(base) = s.strip_suffix("ing").or_else(|| s.strip_suffix("ed")) {
        if base.len() >= 3 {
            s = base.to_string();
        }
    } else if s.len() > 3
        && s.ends_with('s')
        && !s.ends_with("ss")
        && !s.ends_with("us")
        && !s.ends_with("is")
    {
        s.pop();
    }
    if s.len() >= 3 && s.ends_with('e') {
        s.pop();
    }
    let b = s.as_bytes();
    if b.len() > 3 && b[b.len() - 1] == b[b.len() - 2] && !b"aeiou".contains(&b[b.len() - 1]) {
        s.pop();
    }
    s
}

fn is_stopword(word: &str) -> bool {
    EMOJI_STOPWORDS.contains(&word)
}

fn is_soft(word: &str) -> bool {
    EMOJI_SOFT_WORDS.contains(&word)
}

/// A name or keyword: stems of its non-stopword, non-soft words.
type Phrase = Vec<String>;

fn phrase_stems(name: &str) -> Phrase {
    let mut stems: Vec<String> = emoji_key(name)
        .split(' ')
        .filter(|w| !w.is_empty() && !is_stopword(w) && !is_soft(w))
        .map(emoji_stem)
        .collect();
    stems.sort();
    stems.dedup();
    stems
}

struct EmojiEntry {
    emoji: &'static emojis::Emoji,
    names: Vec<Phrase>,
    keywords: Vec<Phrase>,
    /// Stems of every word in any name or keyword, soft words included.
    terms: std::collections::HashSet<String>,
    /// Stems of every word in any name (not keywords), soft words included.
    name_terms: std::collections::HashSet<String>,
    rank: usize,
}

struct EmojiIndex {
    by_name: std::collections::HashMap<String, &'static str>,
    max_words: usize,
    entries: Vec<EmojiEntry>,
    by_term: std::collections::HashMap<String, Vec<usize>>,
}

fn strip_vs16(s: &str) -> String {
    s.replace('\u{fe0f}', "")
}

fn emoji_index() -> &'static EmojiIndex {
    static INDEX: std::sync::OnceLock<EmojiIndex> = std::sync::OnceLock::new();
    INDEX.get_or_init(|| {
        use std::collections::{HashMap, HashSet};
        let mut by_name = HashMap::new();
        let all = || {
            emojis::iter().flat_map(|e| {
                let tones: Vec<&'static emojis::Emoji> =
                    e.skin_tones().map(|t| t.collect()).unwrap_or_default();
                std::iter::once(e).chain(tones.into_iter().skip(1))
            })
        };
        // emoji (without U+FE0F) -> (shortcode names, keywords) from the table.
        let mut table: HashMap<String, (Vec<&'static str>, Vec<&'static str>)> = HashMap::new();
        for line in EMOJI_KEYWORDS.lines().filter(|l| !l.starts_with('#')) {
            let mut cols = line.split('\t');
            let (Some(e), Some(names), Some(keywords)) = (cols.next(), cols.next(), cols.next())
            else {
                continue;
            };
            let split = |s: &'static str| -> Vec<&'static str> {
                s.split('|').filter(|p| !p.is_empty()).collect()
            };
            table.insert(strip_vs16(e), (split(names), split(keywords)));
        }

        // Exact names: aliases > CLDR names > shortcodes.
        for (alias, emoji) in EMOJI_ALIASES {
            by_name.entry(emoji_key(alias)).or_insert(*emoji);
        }
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
        for e in emojis::iter() {
            if let Some((names, _)) = table.get(&strip_vs16(e.as_str())) {
                for name in names {
                    by_name.entry(emoji_key(name)).or_insert(e.as_str());
                }
            }
        }
        let max_words = by_name
            .keys()
            .map(|k| k.split(' ').count())
            .max()
            .unwrap_or(1);

        // Forgiving index over base emoji (skin tones are applied afterwards).
        let mut entries = Vec::new();
        let mut by_term: HashMap<String, Vec<usize>> = HashMap::new();
        for (order, e) in emojis::iter().enumerate() {
            let mut names: Vec<&str> = vec![e.name()];
            names.extend(e.shortcodes());
            names.extend(
                EMOJI_ALIASES
                    .iter()
                    .filter(|(_, a)| strip_vs16(a) == strip_vs16(e.as_str()))
                    .map(|(alias, _)| *alias),
            );
            let (extra_names, keywords) = table
                .get(&strip_vs16(e.as_str()))
                .cloned()
                .unwrap_or_default();
            names.extend(extra_names);

            let stems_of = |list: &[&str]| -> HashSet<String> {
                list.iter()
                    .flat_map(|p| {
                        emoji_key(p)
                            .split(' ')
                            .filter(|w| !w.is_empty() && !is_stopword(w))
                            .map(emoji_stem)
                            .collect::<Vec<_>>()
                    })
                    .collect()
            };
            let name_terms = stems_of(&names);
            let mut terms = stems_of(&keywords);
            terms.extend(name_terms.iter().cloned());
            let to_phrases = |list: &[&str]| -> Vec<Phrase> {
                let mut out: Vec<Phrase> = list
                    .iter()
                    .map(|p| phrase_stems(p))
                    .filter(|p| !p.is_empty())
                    .collect();
                out.sort();
                out.dedup();
                out
            };
            let rank = EMOJI_BY_POPULARITY
                .iter()
                .position(|p| strip_vs16(p) == strip_vs16(e.as_str()))
                .unwrap_or(EMOJI_BY_POPULARITY.len() + order);
            let id = entries.len();
            for term in &terms {
                by_term.entry(term.clone()).or_default().push(id);
            }
            entries.push(EmojiEntry {
                emoji: e,
                names: to_phrases(&names),
                keywords: to_phrases(&keywords),
                terms,
                name_terms,
                rank,
            });
        }
        EmojiIndex {
            by_name,
            max_words,
            entries,
            by_term,
        }
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

fn jaccard(query: &[String], phrase: &[String]) -> f32 {
    let inter = phrase.iter().filter(|p| query.contains(p)).count();
    inter as f32 / (query.len() + phrase.len() - inter) as f32
}

/// Best emoji for spoken name words (already lowercased, punctuation-free),
/// and whether the first word is part of one of its names (vs. a keyword).
fn fuzzy_emoji(name_words: &[String]) -> Option<(&'static emojis::Emoji, bool)> {
    let index = emoji_index();
    let spoken: Vec<String> = name_words
        .iter()
        .flat_map(|w| {
            emoji_key(w)
                .split(' ')
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .filter(|w| !w.is_empty())
        .collect();
    // The first spoken word must carry meaning; leading filler stays text.
    let first = spoken.first()?;
    if is_stopword(first)
        || spoken
            .iter()
            .any(|w| EMOJI_PHRASE_BREAKERS.contains(&w.as_str()))
    {
        return None;
    }
    let words: Vec<&String> = spoken.iter().filter(|w| !is_stopword(w)).collect();
    let mut core: Vec<String> = words
        .iter()
        .filter(|w| !is_soft(w))
        .map(|w| emoji_stem(w))
        .collect();
    core.sort();
    core.dedup();
    let soft: Vec<String> = words
        .iter()
        .filter(|w| is_soft(w))
        .map(|w| emoji_stem(w))
        .collect();
    if core.is_empty() {
        return None;
    }

    // Every meaningful word must appear somewhere in the emoji's names/keywords.
    let mut candidates: Option<Vec<usize>> = None;
    for stem in &core {
        let ids = index.by_term.get(stem)?;
        candidates = Some(match candidates {
            None => ids.clone(),
            Some(prev) => prev.into_iter().filter(|id| ids.contains(id)).collect(),
        });
    }

    let mut best: Option<(f32, &EmojiEntry)> = None;
    for id in candidates? {
        let entry = &index.entries[id];
        let base = entry
            .names
            .iter()
            .map(|p| jaccard(&core, p))
            .chain(
                entry
                    .keywords
                    .iter()
                    .map(|p| EMOJI_KEYWORD_WEIGHT * jaccard(&core, p)),
            )
            .fold(0.0_f32, f32::max);
        if base < EMOJI_MIN_SCORE {
            continue;
        }
        let bonus = soft.iter().filter(|s| entry.terms.contains(*s)).count() as f32;
        let score = base + EMOJI_SOFT_BONUS * bonus;
        let better = match best {
            None => true,
            Some((s, b)) => {
                score > s + f32::EPSILON
                    || ((score - s).abs() <= f32::EPSILON && entry.rank < b.rank)
            }
        };
        if better {
            best = Some((score, entry));
        }
    }
    let (_, entry) = best?;
    // Leading filler ("big smile") belongs to the name.
    let first_named = is_soft(first) || entry.name_terms.contains(&emoji_stem(first));
    Some((entry.emoji, first_named))
}

/// The emoji, optionally re-toned; falls back to the plain emoji when the tone
/// does not apply.
fn with_tone(emoji: &str, tone: Option<emojis::SkinTone>) -> (String, bool) {
    if let Some(tone) = tone {
        if let Some(toned) = emojis::get(emoji).and_then(|e| e.with_skin_tone(tone)) {
            return (toned.as_str().to_string(), true);
        }
    }
    (emoji.to_string(), false)
}

/// Mishearings of "emoji" that are not English words, so accepting them as the
/// trigger cannot rewrite normal speech. Only accepted where the trigger ends
/// the phrase right after a recognised emoji name. Deliberately absent:
/// "emergency" and "India", which speech models also produce for "emoji" but
/// which are common words ("we had a family emergency" must stay text).
const EMOJI_TRIGGER_VARIANTS: &[&str] = &[
    "emojy", "emojie", "emoje", "emojii", "emogi", "emogee", "emoji's", "imoji", "imogi", "imojis",
];

/// Two-word splits of "emoji" ("a moji", "emo g"), under the same rule.
const EMOJI_TRIGGER_SPLITS: &[(&str, &str)] = &[
    ("a", "moji"),
    ("e", "moji"),
    ("emo", "g"),
    ("emo", "gee"),
    ("emo", "ji"),
    ("emo", "jee"),
    ("imo", "g"),
    ("imo", "ji"),
];

/// A spoken trigger starting at `j`: its length in words, and whether it is
/// the word "emoji" itself.
fn trigger_at(words: &[String], j: usize) -> Option<(usize, bool)> {
    let w = words.get(j)?.as_str();
    if w == EMOJI_WORD {
        return Some((1, true));
    }
    if EMOJI_TRIGGER_VARIANTS.contains(&w) {
        return Some((1, false));
    }
    let next = words.get(j + 1)?.as_str();
    EMOJI_TRIGGER_SPLITS
        .iter()
        .any(|&(a, b)| w == a && next == b)
        .then_some((2, false))
}

/// Words a clipped or repeated start of the trigger leaves right before it
/// ("party emo emoji", "tongue out a mo emoji"): how many of `words[i..e]` to
/// fold into the trigger. At least one name word always remains.
fn trigger_fragment_before(words: &[String], i: usize, e: usize) -> usize {
    let w = |k: usize| words[k].as_str();
    if e >= i + 3 && matches!((w(e - 2), w(e - 1)), ("a", "mo") | ("e", "mo")) {
        return 2;
    }
    if e >= i + 2 && matches!(w(e - 1), "emo" | "imo" | "emoj") {
        return 1;
    }
    0
}

/// `<name words> emoji [skin tone]` starting at `i`: the emoji and words used.
fn match_emoji(words: &[String], tokens: &[&str], i: usize) -> Option<(String, usize)> {
    let index = emoji_index();
    if words[i] == "emojis" || trigger_at(words, i).is_some() {
        return None;
    }
    // The nearest trigger after `i`; the name must not contain another one.
    let (t, trigger) = (i + 1..words.len().min(i + index.max_words + 1)).find_map(|j| {
        if words[j] == "emojis" {
            Some((j, None))
        } else {
            trigger_at(words, j).map(|found| (j, Some(found)))
        }
    })?;
    let (trigger_len, exact) = trigger?;
    let fragment = trigger_fragment_before(words, i, t);
    // The name is words[i..e]; everything from e to `after` is the trigger.
    let e = t - fragment;
    let after = t + trigger_len;
    let n = e - i;
    let used = after - i;
    let after_tone = match_skin_tone(words, after);
    let finish = |emoji: &str, used: usize| {
        let (out, toned) = with_tone(emoji, after_tone.map(|t| t.0));
        let extra = if toned {
            after_tone.map_or(0, |t| t.1)
        } else {
            0
        };
        (out, used + extra)
    };

    let ends_phrase = after == words.len()
        || tokens[after - 1].ends_with(PUNCTUATION)
        || after_tone.is_some()
        || match_at(words, after).is_some();
    let plain_trigger = exact && fragment == 0;
    if !plain_trigger {
        // A misheard trigger only counts at the end of a phrase, in one piece,
        // after a name (not "a"/"the"/"my" ...).
        if !ends_phrase
            || tokens[e..after - 1]
                .iter()
                .any(|t| t.ends_with(PUNCTUATION))
        {
            return None;
        }
        let before = emoji_key(&words[e - 1]);
        if EMOJI_NOUN_MARKERS.contains(&before.as_str()) || is_stopword(&before) {
            return None;
        }
    }

    if let Some(emoji) = joined_key(words, i, n).and_then(|k| index.by_name.get(&k).copied()) {
        return Some(finish(emoji, used));
    }

    // Forgiving match: only where the trigger ends the phrase.
    if !ends_phrase || n > EMOJI_MAX_FUZZY_WORDS {
        return None;
    }
    let before = emoji_key(&words[e - 1]);
    if EMOJI_NOUN_MARKERS.contains(&before.as_str()) || is_stopword(&before) {
        return None;
    }
    // The name must not span a sentence or clause break.
    if tokens[i..e - 1].iter().any(|t| t.ends_with(PUNCTUATION)) {
        return None;
    }
    // "<name> <tone> skin tone emoji".
    let mut name_end = e;
    let mut tone = None;
    for t in (2..=4).rev() {
        if name_end >= i + 1 + t {
            if let Some((found, used)) = match_skin_tone(words, e - t) {
                if used == t {
                    tone = Some(found);
                    name_end = e - t;
                    break;
                }
            }
        }
    }
    let (emoji, first_named) = fuzzy_emoji(&words[i..name_end])?;
    let result = match tone {
        Some(_) => (with_tone(emoji.as_str(), tone).0, used),
        None => finish(emoji.as_str(), used),
    };
    // A leading word that is only a keyword ("hilarious" for 😂) stays text
    // when the rest of the phrase gives the same emoji.
    if !first_named {
        if let Some((rest, _)) = match_emoji(words, tokens, i + 1) {
            if strip_vs16(&rest) == strip_vs16(&result.0) {
                return None;
            }
        }
    }
    Some(result)
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
        if let Some((emoji_str, len)) = emoji.then(|| match_emoji(&words, &tokens, i)).flatten() {
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

/// Emoji-only pass for text that already went through post-processing, so a
/// word the LLM corrected ("birthday party emergency" -> "birthday party
/// emoji") can still become the emoji. Works line by line so the LLM's line
/// breaks survive; text with no "<name> emoji" phrase comes back unchanged.
pub fn apply_voice_emoji(text: &str) -> String {
    text.split('\n')
        .map(|line| apply_voice_commands(line, false, true).text)
        .collect::<Vec<_>>()
        .join("\n")
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
    fn spoken_emoji_forgiving() {
        for (text, want) in [
            ("sticking tongue out emoji", "😛"),
            ("Sticking tongue out emoji.", "😛."),
            ("tongue out emoji", "😛"),
            ("tongue out wink emoji", "😜"),
            ("winking tongue out emoji", "😜"),
            ("laughing crying emoji", "😂"),
            ("crying laughing emoji", "😂"),
            ("laughing emoji", "😂"),
            ("thumbs up emoji", "👍"),
            ("Thumbs up, emoji!", "👍!"),
            ("smiley emoji", "😃"),
            ("smiley face emoji", "😃"),
            ("heart eyes emoji", "😍"),
            ("red heart emoji", "❤️"),
            ("fire emoji", "🔥"),
            ("party emoji", "🥳"),
            ("eye roll emoji", "🙄"),
            ("eye-roll emoji", "🙄"),
            ("eyes rolling emoji", "🙄"),
            ("skull emoji", "💀"),
            ("praying hands emoji", "🙏"),
            ("clapping emoji", "👏"),
            ("clapping hands emoji", "👏"),
            ("hundred emoji", "💯"),
            ("100 emoji", "💯"),
            ("shrug emoji", "🤷"),
            ("shrugging emoji", "🤷"),
            ("facepalm emoji", "🤦"),
            ("sad face emoji", "😢"),
            ("angry emoji", "😠"),
            ("angry face emoji", "😠"),
            ("wink emoji", "😉"),
            ("winking face emoji", "😉"),
            ("kiss emoji", "😘"),
            ("thinking emoji", "🤔"),
            ("thinking face emoji", "🤔"),
            ("wheelchair emoji", "♿"),
            ("heart on fire emoji", "❤️‍🔥"),
            ("pleading emoji", "🥺"),
            ("upside down emoji", "🙃"),
            ("sweat smile emoji", "😅"),
            ("blushing emoji", "😊"),
            ("saluting emoji", "🫡"),
            ("melting face emoji", "🫠"),
            ("mind blown emoji", "🤯"),
            ("sunglasses face emoji", "😎"),
            ("cool emoji", "😎"),
            ("big smile emoji", "😄"),
            ("ghost emoji", "👻"),
            ("throwing up emoji", "🤮"),
            ("power wheelchair emoji", "🧑‍🦼"),
            ("motorized wheelchair emoji", "🦼"),
            ("person in wheelchair emoji", "🧑‍🦼"),
            ("winking tongue emoji", "😜"),
            ("loudly crying emoji", "😭"),
            ("exploding head emoji", "🤯"),
            ("fingers crossed emoji", "🤞"),
            ("raised hands emoji", "🙌"),
            ("rolling on the floor laughing emoji", "🤣"),
        ] {
            assert_eq!(run(text), want, "{text}");
        }
    }

    #[test]
    fn spoken_emoji_in_sentence() {
        // Josh's real dictation (Parakeet raw, Oct 26 2026).
        assert_eq!(
            run("Some of the it popped up on my screen I'm trying the emojis sticking tongue out emoji"),
            "Some of the it popped up on my screen I'm trying the emojis 😛"
        );
        assert_eq!(
            run("I'm trying the sticking tongue out emoji."),
            "I'm trying the 😛."
        );
        assert_eq!(
            run("that's hilarious laughing crying emoji"),
            "that's hilarious 😂"
        );
        assert_eq!(run("see you soon. Waving hand emoji"), "see you soon. 👋");
        assert_eq!(
            run("great job clapping emoji dark skin tone"),
            "great job 👏🏿"
        );
        assert_eq!(run("so funny dying laughing emoji"), "so funny dying 😂");
        assert_eq!(run("gross throwing up emoji"), "gross 🤮");
        assert_eq!(run("tongue out wink emoji new line ok"), "😜\nOk");
        // Josh's 23:04 recording, as the full (batch) decode hears it.
        assert_eq!(
            run("Okay, I'm testing it now birthday party emoji"),
            "Okay, I'm testing it now 🎉"
        );
    }

    #[test]
    fn forgiving_emoji_leaves_speech_alone() {
        for text in [
            "my heart is on fire",
            "I sent you an emoji",
            "I love that emoji",
            "what is your favorite emoji",
            "the emoji was cute",
            "unknown thing emoji",
            "I'm trying the emojis",
            "send it to me emoji",
        ] {
            assert_eq!(run(text), text, "{text}");
        }
    }

    #[test]
    fn misheard_trigger_after_a_name() {
        for (text, want) in [
            ("party emojy", "🥳"),
            ("fire a moji", "🔥"),
            ("Fire, a moji.", "🔥."),
            ("thumbs up emo g", "👍"),
            ("red heart imoji.", "❤️."),
            ("skull emoji's", "💀"),
            ("thumbs up emojy medium skin tone", "👍🏽"),
            // A clipped or repeated start of the word before it.
            ("party emo emoji", "🥳"),
            ("sticking tongue out a mo emoji", "😛"),
            ("great job clapping emo emoji", "great job 👏"),
        ] {
            assert_eq!(run(text), want, "{text}");
        }
    }

    #[test]
    fn misheard_trigger_leaves_speech_alone() {
        for text in [
            // Common words a model may produce for "emoji" are never the trigger.
            "We had a family emergency",
            "there's a fire emergency.",
            "Is this an emergency?",
            "I'm going to India",
            "birthday party emergency",
            // Variants only count at the end of a phrase, after a name.
            "the fire emojy was cute",
            "I like emo music",
            "my emo phase",
            "the a moji",
            "send me a moji",
            // Josh's live-stream transcript (Oct 26 2026): no name before it.
            "India still doesn't work took a mo emoji as emergency",
        ] {
            assert_eq!(run(text), text, "{text}");
        }
    }

    #[test]
    fn emoji_stemmer() {
        for (a, b) in [
            ("smiling", "smile"),
            ("smiled", "smile"),
            ("eyes", "eye"),
            ("clapping", "clap"),
            ("cries", "cry"),
            ("rolling", "roll"),
            ("kissing", "kiss"),
            ("stuck", "sticking"),
            ("glasses", "glass"),
        ] {
            assert_eq!(emoji_stem(a), emoji_stem(b), "{a} / {b}");
        }
        assert_eq!(emoji_stem("red"), "red");
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

    #[test]
    fn emoji_pass_after_post_processing() {
        // What Luna returns after fixing a misheard trigger.
        assert_eq!(
            apply_voice_emoji("Okay, I'm testing it now. Birthday party emoji."),
            "Okay, I'm testing it now. 🎉."
        );
        assert_eq!(
            apply_voice_emoji("Sounds good to me, thumbs up emoji"),
            "Sounds good to me, 👍"
        );
        // Line breaks from the LLM survive.
        assert_eq!(
            apply_voice_emoji("Hi Sam,\n\nSee you soon fire emoji\nJosh"),
            "Hi Sam,\n\nSee you soon 🔥\nJosh"
        );
        // Emoji the LLM already wrote, and plain text, pass through unchanged.
        for text in [
            "That's hilarious 😂 see you tomorrow.",
            "We had a family emergency last night.",
            "I'm going to India next month.",
            "Emoji still doesn't work. Took emoji as emergency.",
            "  Hello there,\n  world.  ",
            "",
        ] {
            assert_eq!(apply_voice_emoji(text), text);
        }
        // Running it twice changes nothing more.
        let once = apply_voice_emoji("party emoji and heart emoji");
        assert_eq!(apply_voice_emoji(&once), once);
    }
}
