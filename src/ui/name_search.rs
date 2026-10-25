//! Name search for the Build Editor's lists, in whatever language the names
//! are: a name matches when it starts with what was typed, letter case aside.
//! Typing is met halfway in three ways. A plain letter matches that letter
//! under any accent ("e" finds "Épée", while "é" still finds only "é"); a lone
//! Korean initial consonant matches the syllables it begins (ㄱ finds 가 and
//! 검), as Korean search boxes do; and hiragana matches the same kana written
//! in katakana, the way a Japanese IME types before converting.

// Whether `name` starts with `typed`, as above.
pub(crate) fn starts_with(name: &str, typed: &str) -> bool {
    let mut name = name.chars().flat_map(char::to_lowercase);
    typed
        .chars()
        .flat_map(char::to_lowercase)
        .all(|letter| name.next().is_some_and(|shown| same(letter, shown)))
}

// What to sort names by: lowercased with the accents off, so "Épée" files
// under E rather than after Z.
pub(crate) fn sort_key(name: &str) -> String {
    name.chars()
        .flat_map(char::to_lowercase)
        .map(|c| plain_letter(c).unwrap_or(c))
        .collect()
}

// Whether a lowercased typed character stands for a lowercased shown one.
fn same(typed: char, shown: char) -> bool {
    typed == shown
        || (typed.is_ascii_alphabetic() && plain_letter(shown) == Some(typed))
        || hangul_initial(shown) == Some(typed)
        || katakana(typed) == Some(shown)
}

// The plain letters of U+00C0..=U+017F (Latin-1 and Latin Extended-A) and
// U+1EA0..=U+1EF9 (Vietnamese), one a code point; `.` for one that is not a
// letter with a plain form. Made from Unicode's decompositions.
const LATIN: &[u8] = b"aaaaaaaceeeeiiiidnooooo.ouuuuy.saaaaaaaceeeeiiiidnooooo.ouuuuy.y\
aaaaaaccccccccddddeeeeeeeeeegggggggghhhhiiiiiiiiiiiijjkkkllllllllllnnnnnnnnnoooooooorrrrrrssssssssttttttuuuuuuuuuuuuwwyyyzzzzzzs";
const VIETNAMESE: &[u8] =
    b"aaaaaaaaaaaaaaaaaaaaaaaaeeeeeeeeeeeeeeeeiiiioooooooooooooooooooooooouuuuuuuuuuuuuuyyyyyyyy";

// The plain letter an accented Latin letter is written on, if it is one.
fn plain_letter(c: char) -> Option<char> {
    let code = c as u32;
    let letter = match code {
        0xC0..=0x17F => LATIN[(code - 0xC0) as usize],
        0x1EA0..=0x1EF9 => VIETNAMESE[(code - 0x1EA0) as usize],
        // Vietnamese's horned O and U, alone in Latin Extended-B.
        0x1A0 | 0x1A1 => b'o',
        0x1AF | 0x1B0 => b'u',
        _ => return None,
    };
    (letter != b'.').then_some(char::from(letter))
}

// The consonants a Hangul syllable can begin with, in the order Unicode
// composes syllables by, as a keyboard types them alone (compatibility jamo).
const HANGUL_INITIALS: [char; 19] = [
    'ㄱ', 'ㄲ', 'ㄴ', 'ㄷ', 'ㄸ', 'ㄹ', 'ㅁ', 'ㅂ', 'ㅃ', 'ㅅ', 'ㅆ', 'ㅇ', 'ㅈ', 'ㅉ', 'ㅊ', 'ㅋ',
    'ㅌ', 'ㅍ', 'ㅎ',
];

// The consonant a precomposed Hangul syllable begins with (U+AC00..=U+D7A3:
// 19 initials x 21 vowels x 28 finals).
fn hangul_initial(c: char) -> Option<char> {
    let index = (c as u32)
        .checked_sub(0xAC00)
        .filter(|index| *index < 19 * 21 * 28)?;
    HANGUL_INITIALS.get((index / (21 * 28)) as usize).copied()
}

// The katakana for a hiragana, which sits 0x60 above it.
fn katakana(c: char) -> Option<char> {
    matches!(c, '\u{3041}'..='\u{3096}')
        .then(|| char::from_u32(c as u32 + 0x60))
        .flatten()
}
