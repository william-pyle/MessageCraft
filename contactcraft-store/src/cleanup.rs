//! Tidying contacts: names in proper case and without stray symbols, phone numbers and addresses
//! in one standard form, no repeated emails or numbers on a contact, and finding contacts that are
//! the same person (by name or number) so they can be merged.
//!
//! Everything here is pure: it changes `Contact` values and leaves saving to the caller.

use std::collections::{HashMap, HashSet};

use serde_json::{Map, Value};

use crate::model::*;

/// Which clean-up steps to run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    pub proper_case: bool,
    pub strip_symbols: bool,
    pub phones: bool,
    pub addresses: bool,
    pub dedupe_emails: bool,
    pub dedupe_phones: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options { proper_case: true, strip_symbols: true, phones: true, addresses: true, dedupe_emails: true, dedupe_phones: true }
    }
}

/// What a clean-up changed on one contact.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Changes {
    pub names: bool,
    pub phones: usize,
    pub addresses: usize,
    pub emails_removed: usize,
    pub phones_removed: usize,
}

impl Changes {
    pub fn any(&self) -> bool {
        self.names || self.phones > 0 || self.addresses > 0 || self.emails_removed > 0 || self.phones_removed > 0
    }
}

/// Tidy one contact.
pub fn clean(c: &mut Contact, o: &Options) -> Changes {
    let mut ch = Changes::default();
    if o.strip_symbols || o.proper_case {
        let before = c.name.clone();
        let n = &mut c.name;
        let fields: [(&mut String, NamePart); 12] = [
            (&mut n.prefix, NamePart::Prefix),
            (&mut n.given, NamePart::Given),
            (&mut n.middle, NamePart::Family),
            (&mut n.family, NamePart::Family),
            (&mut n.suffix, NamePart::Suffix),
            (&mut n.nickname, NamePart::Given),
            (&mut n.maiden, NamePart::Family),
            (&mut n.phonetic_given, NamePart::Given),
            (&mut n.phonetic_middle, NamePart::Given),
            (&mut n.phonetic_family, NamePart::Family),
            (&mut n.display, NamePart::Display),
            (&mut n.file_as, NamePart::FileAs),
        ];
        for (f, part) in fields {
            let mut v = f.clone();
            if o.strip_symbols {
                v = strip_symbols(&v, part == NamePart::FileAs || part == NamePart::Display);
            }
            if o.proper_case {
                v = proper_case(&v, part);
            }
            *f = v;
        }
        if o.strip_symbols {
            let i = strip_symbols(&n.initials, false).replace(['.', ' '], "");
            n.initials = i.to_uppercase();
        }
        ch.names = c.name != before;
    }
    if o.phones {
        for p in c.phones.iter_mut() {
            let f = format_phone(&p.value);
            if f != p.value {
                p.value = f;
                ch.phones += 1;
            }
        }
    }
    if o.addresses {
        for a in c.addresses.iter_mut() {
            let f = format_address(a);
            if f != *a {
                *a = f;
                ch.addresses += 1;
            }
        }
    }
    if o.dedupe_emails {
        ch.emails_removed = dedupe_emails(&mut c.emails);
    }
    if o.dedupe_phones {
        ch.phones_removed = dedupe_phones(&mut c.phones);
    }
    ch
}

// --- names -----------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NamePart {
    Prefix,
    Given,
    Family,
    Suffix,
    Display,
    FileAs,
}

/// Remove symbols, digits and emoji from a name, keeping letters (of any alphabet, with their
/// accents), spaces, hyphens, apostrophes and full stops (and commas where they belong).
pub fn strip_symbols(s: &str, keep_commas: bool) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        let ch = match ch {
            '’' | '‘' | '`' | '´' => '\'',
            '‐' | '‑' | '–' | '—' => '-',
            c => c,
        };
        let mark = ('\u{300}'..='\u{36F}').contains(&ch);
        if ch.is_alphabetic() || mark || matches!(ch, ' ' | '-' | '\'' | '.') || (keep_commas && ch == ',') {
            out.push(ch);
        } else if ch.is_whitespace() || ch == '_' || ch == '/' {
            out.push(' ');
        }
    }
    // Tidy what's left: single spaces, no repeated punctuation, nothing dangling at the ends.
    let mut tidy = String::new();
    let mut prev = ' ';
    for ch in out.chars() {
        if (ch == ' ' && prev == ' ') || (matches!(ch, '-' | '\'' | '.' | ',') && ch == prev) {
            continue;
        }
        if ch == ' ' && matches!(prev, '-' | '\'') {
            continue;
        }
        if matches!(ch, '-' | '\'') && prev == ' ' && tidy.ends_with(' ') {
            tidy.pop();
        }
        tidy.push(ch);
        prev = ch;
    }
    tidy.trim_matches(|c: char| c == ' ' || c == '-' || c == '\'' || c == ',').trim_start_matches('.').trim().to_string()
}

const PARTICLES: &[&str] = &["van", "von", "der", "den", "de", "del", "della", "da", "di", "du", "la", "le", "bin", "ibn", "al", "dos", "das", "ter", "ten"];
const SUFFIXES: &[(&str, &str)] = &[
    ("jr", "Jr"),
    ("sr", "Sr"),
    ("ii", "II"),
    ("iii", "III"),
    ("iv", "IV"),
    ("v", "V"),
    ("vi", "VI"),
    ("phd", "PhD"),
    ("md", "MD"),
    ("dds", "DDS"),
    ("dmd", "DMD"),
    ("do", "DO"),
    ("rn", "RN"),
    ("cpa", "CPA"),
    ("esq", "Esq"),
    ("mba", "MBA"),
    ("jd", "JD"),
    ("ret", "Ret"),
];

fn cap_first(w: &str) -> String {
    let mut c = w.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// One word in proper case: "SMITH" → "Smith", "o'brien" → "O'Brien", "mcdonald" → "McDonald",
/// "anne-marie" → "Anne-Marie". A word that already mixes capitals and small letters ("DeShawn",
/// "MacKenzie") is left as it is, since that's how the person spells it.
fn proper_word(w: &str) -> String {
    let has_upper = w.chars().any(|c| c.is_uppercase());
    let has_lower = w.chars().any(|c| c.is_lowercase());
    if has_upper && has_lower {
        return w.to_string();
    }
    // Capital initials with no vowel ("DJ", "TJ") stay as they are.
    let letters: Vec<char> = w.chars().filter(|c| c.is_alphabetic()).collect();
    if has_upper && letters.len() == 2 && !letters.iter().any(|c| "AEIOUY".contains(*c)) {
        return w.to_string();
    }
    let lower = w.to_lowercase();
    let mut out = String::new();
    let mut start = true;
    for ch in lower.chars() {
        if start && ch.is_alphabetic() {
            out.extend(ch.to_uppercase());
            start = false;
        } else {
            out.push(ch);
        }
        if ch == '-' || ch == '\'' || ch == '.' {
            start = true;
        }
    }
    // Mc+name keeps its second capital.
    if out.len() > 3 && out.starts_with("Mc") {
        let rest: String = out.chars().skip(2).collect();
        out = format!("Mc{}", cap_first(&rest));
    }
    // A single apostrophe-s ("Smith's") stays small.
    if let Some(base) = out.strip_suffix("'S") {
        out = format!("{base}'s");
    }
    out
}

/// A name field in proper case.
pub fn proper_case(s: &str, part: NamePart) -> String {
    let words: Vec<&str> = s.split(' ').filter(|w| !w.is_empty()).collect();
    let many = words.len() > 1;
    words
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let bare = w.trim_end_matches(['.', ',']).to_lowercase();
            let tail = &w[w.trim_end_matches(['.', ',']).len()..];
            if let Some((_, fixed)) = SUFFIXES.iter().find(|(k, _)| *k == bare) {
                // "v" and "do" are only suffixes in the suffix field (or after a name).
                if part == NamePart::Suffix || (i > 0 && !matches!(bare.as_str(), "v" | "do")) {
                    return format!("{fixed}{tail}");
                }
            }
            if matches!(part, NamePart::Family | NamePart::Display | NamePart::FileAs) && many && i + 1 < words.len() && PARTICLES.contains(&bare.as_str()) && i > 0 {
                return w.to_lowercase();
            }
            if part == NamePart::Family && many && i == 0 && PARTICLES.contains(&bare.as_str()) && w.chars().all(|c| !c.is_uppercase()) {
                return w.to_string();
            }
            proper_word(w)
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// --- phone numbers -----------------------------------------------------------------------------

/// A phone number in one standard form: `(555) 010-2030` for ten-digit North American numbers,
/// `+1 (555) 010-2030` with the country code, `555-0102` for seven digits; other international
/// numbers keep their own grouping as `+44 7700 900123`. Extensions become ` ext. 123`.
/// Short codes and anything that isn't a number are left alone.
pub fn format_phone(s: &str) -> String {
    let t = s.trim();
    if t.is_empty() {
        return String::new();
    }
    // Split off an extension: "x123", "ext 123", "ext. 123", "#123", ",123" (pause).
    let lower = t.to_lowercase();
    let mut main = t.to_string();
    let mut ext = String::new();
    for marker in [" ext.", " ext", "ext.", "ext", " x", "x", "#", ","] {
        if let Some(i) = lower.find(marker) {
            if i > 0 && lower[..i].chars().filter(|c| c.is_ascii_digit()).count() >= 7 {
                let digits: String = lower[i + marker.len()..].chars().filter(|c| c.is_ascii_digit()).collect();
                if !digits.is_empty() {
                    ext = digits;
                    main = t[..i].to_string();
                    break;
                }
            }
        }
    }
    if main.chars().any(|c| c.is_alphabetic()) {
        return t.to_string();
    }
    let plus = main.trim_start().starts_with('+') || main.trim_start().starts_with("00");
    let mut digits: String = main.chars().filter(|c| c.is_ascii_digit()).collect();
    if main.trim_start().starts_with("00") {
        digits = digits[2..].to_string();
    }
    let nanp = |d: &str| format!("({}) {}-{}", &d[0..3], &d[3..6], &d[6..10]);
    let mut out = if !plus && digits.len() == 10 {
        nanp(&digits)
    } else if digits.len() == 11 && digits.starts_with('1') {
        format!("+1 {}", nanp(&digits[1..]))
    } else if !plus && digits.len() == 7 {
        format!("{}-{}", &digits[0..3], &digits[3..7])
    } else if plus && digits.len() >= 8 {
        // Keep the number's own groups, separated by single spaces; drop a "(0)".
        let body = main.trim().trim_start_matches('+').replace("(0)", " ");
        let groups: Vec<String> = body.split(|c: char| !c.is_ascii_digit()).filter(|g| !g.is_empty()).map(String::from).collect();
        let mut g = groups.join(" ");
        if main.trim_start().starts_with("00") {
            g = g.trim_start_matches("00").trim().to_string();
        }
        if groups.len() <= 1 { format!("+{digits}") } else { format!("+{g}") }
    } else {
        return t.split_whitespace().collect::<Vec<_>>().join(" ");
    };
    if !ext.is_empty() {
        out.push_str(&format!(" ext. {ext}"));
    }
    out
}

// --- addresses ---------------------------------------------------------------------------------

const STREET_WORDS: &[(&str, &str)] = &[
    ("street", "St"),
    ("st", "St"),
    ("avenue", "Ave"),
    ("ave", "Ave"),
    ("av", "Ave"),
    ("road", "Rd"),
    ("rd", "Rd"),
    ("drive", "Dr"),
    ("dr", "Dr"),
    ("boulevard", "Blvd"),
    ("blvd", "Blvd"),
    ("lane", "Ln"),
    ("ln", "Ln"),
    ("court", "Ct"),
    ("ct", "Ct"),
    ("place", "Pl"),
    ("pl", "Pl"),
    ("terrace", "Ter"),
    ("circle", "Cir"),
    ("highway", "Hwy"),
    ("hwy", "Hwy"),
    ("parkway", "Pkwy"),
    ("pkwy", "Pkwy"),
    ("square", "Sq"),
    ("trail", "Trl"),
    ("expressway", "Expy"),
    ("freeway", "Fwy"),
    ("suite", "Ste"),
    ("ste", "Ste"),
    ("apartment", "Apt"),
    ("apt", "Apt"),
    ("building", "Bldg"),
    ("bldg", "Bldg"),
    ("floor", "Fl"),
    ("room", "Rm"),
    ("ne", "NE"),
    ("nw", "NW"),
    ("se", "SE"),
    ("sw", "SW"),
    ("po", "PO"),
];

const US_STATES: &[(&str, &str)] = &[
    ("alabama", "AL"), ("alaska", "AK"), ("arizona", "AZ"), ("arkansas", "AR"), ("california", "CA"), ("colorado", "CO"), ("connecticut", "CT"), ("delaware", "DE"),
    ("district of columbia", "DC"), ("florida", "FL"), ("georgia", "GA"), ("hawaii", "HI"), ("idaho", "ID"), ("illinois", "IL"), ("indiana", "IN"), ("iowa", "IA"),
    ("kansas", "KS"), ("kentucky", "KY"), ("louisiana", "LA"), ("maine", "ME"), ("maryland", "MD"), ("massachusetts", "MA"), ("michigan", "MI"), ("minnesota", "MN"),
    ("mississippi", "MS"), ("missouri", "MO"), ("montana", "MT"), ("nebraska", "NE"), ("nevada", "NV"), ("new hampshire", "NH"), ("new jersey", "NJ"), ("new mexico", "NM"),
    ("new york", "NY"), ("north carolina", "NC"), ("north dakota", "ND"), ("ohio", "OH"), ("oklahoma", "OK"), ("oregon", "OR"), ("pennsylvania", "PA"), ("rhode island", "RI"),
    ("south carolina", "SC"), ("south dakota", "SD"), ("tennessee", "TN"), ("texas", "TX"), ("utah", "UT"), ("vermont", "VT"), ("virginia", "VA"), ("washington", "WA"),
    ("west virginia", "WV"), ("wisconsin", "WI"), ("wyoming", "WY"), ("puerto rico", "PR"), ("guam", "GU"), ("virgin islands", "VI"),
];

const COUNTRIES: &[(&[&str], &str, &str)] = &[
    (&["us", "usa", "u.s.", "u.s.a.", "united states", "united states of america", "america"], "United States", "US"),
    (&["ca", "can", "canada"], "Canada", "CA"),
    (&["uk", "u.k.", "gb", "great britain", "united kingdom", "england", "scotland", "wales"], "United Kingdom", "GB"),
    (&["au", "aus", "australia"], "Australia", "AU"),
    (&["nz", "new zealand"], "New Zealand", "NZ"),
    (&["ie", "ireland"], "Ireland", "IE"),
    (&["de", "germany", "deutschland"], "Germany", "DE"),
    (&["fr", "france"], "France", "FR"),
    (&["es", "spain", "españa"], "Spain", "ES"),
    (&["it", "italy", "italia"], "Italy", "IT"),
    (&["mx", "mexico", "méxico"], "Mexico", "MX"),
    (&["nl", "netherlands", "the netherlands", "holland"], "Netherlands", "NL"),
    (&["in", "india"], "India", "IN"),
    (&["jp", "japan"], "Japan", "JP"),
    (&["cn", "china"], "China", "CN"),
    (&["br", "brazil", "brasil"], "Brazil", "BR"),
    (&["ph", "philippines"], "Philippines", "PH"),
];

fn tidy_spaces(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Proper case only for text written all in capitals or all small letters.
fn case_if_flat(s: &str) -> String {
    let flat = !(s.chars().any(|c| c.is_uppercase()) && s.chars().any(|c| c.is_lowercase()));
    if !flat {
        return s.to_string();
    }
    s.split(' ')
        .map(|w| if w.chars().next().is_some_and(|c| c.is_ascii_digit()) { w.to_lowercase() } else { proper_word(w) })
        .collect::<Vec<_>>()
        .join(" ")
}

fn street_line(line: &str) -> String {
    let line = case_if_flat(&tidy_spaces(line).replace(" ,", ","));
    line.split(' ')
        .map(|w| {
            let bare = w.trim_end_matches(['.', ',']).to_lowercase();
            let comma = if w.ends_with(',') { "," } else { "" };
            match STREET_WORDS.iter().find(|(k, _)| *k == bare) {
                Some((_, std)) => format!("{std}{comma}"),
                None => w.to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
        .trim_end_matches(',')
        .to_string()
}

/// An address in one standard form: tidy spacing and capitals, standard street words (St, Ave,
/// Rd, Apt, Ste...), US states as their two-letter codes, ZIP+4 as 12345-6789, Canadian postal
/// codes as A1A 1A1, and one name per country (with its code).
pub fn format_address(a: &Address) -> Address {
    let mut b = a.clone();
    b.street = a.street.lines().map(street_line).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n");
    b.extended = a.extended.lines().map(street_line).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n");
    let po = tidy_spaces(&a.po_box);
    let po_lower = po.to_lowercase().replace('.', "");
    b.po_box = po_lower.strip_prefix("po box").or_else(|| po_lower.strip_prefix("box")).map(|r| r.trim().to_uppercase()).unwrap_or(po);
    b.city = case_if_flat(&tidy_spaces(a.city.trim_end_matches(',')));
    let country_in = tidy_spaces(&a.country).to_lowercase();
    let found = COUNTRIES.iter().find(|(names, _, _)| names.contains(&country_in.as_str()));
    if let Some((_, name, code)) = found {
        b.country = name.to_string();
        if b.country_code.trim().is_empty() || b.country_code.eq_ignore_ascii_case(code) {
            b.country_code = code.to_string();
        }
    } else {
        b.country = case_if_flat(&tidy_spaces(&a.country));
        b.country_code = a.country_code.trim().to_uppercase();
    }
    let us = b.country_code == "US" || (b.country.is_empty() && b.country_code.is_empty());
    let region = tidy_spaces(a.region.trim_end_matches('.'));
    let rl = region.to_lowercase();
    b.region = if us {
        US_STATES.iter().find(|(n, _)| *n == rl).map(|(_, c)| c.to_string()).unwrap_or_else(|| if region.len() == 2 { region.to_uppercase() } else { case_if_flat(&region) })
    } else if region.len() <= 3 && region.chars().all(|c| c.is_ascii_alphabetic()) {
        region.to_uppercase()
    } else {
        case_if_flat(&region)
    };
    let pc = tidy_spaces(&a.postal_code);
    let digits: String = pc.chars().filter(|c| c.is_ascii_digit()).collect();
    let compact: String = pc.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_uppercase();
    b.postal_code = if us && digits.len() == 9 && digits.len() == pc.chars().filter(|c| c.is_ascii_alphanumeric()).count() {
        format!("{}-{}", &digits[0..5], &digits[5..9])
    } else if b.country_code == "CA" && compact.len() == 6 {
        format!("{} {}", &compact[0..3], &compact[3..6])
    } else if b.country_code == "GB" && compact.len() >= 5 && compact.len() <= 7 {
        format!("{} {}", &compact[..compact.len() - 3], &compact[compact.len() - 3..])
    } else {
        pc.to_uppercase()
    };
    b.label = a.label.trim().to_string();
    b
}

// --- repeats on one contact --------------------------------------------------------------------

/// Remove repeated email addresses (case doesn't matter) and empty ones. Returns how many went.
pub fn dedupe_emails(list: &mut Vec<Email>) -> usize {
    let before = list.len();
    let mut seen: Vec<(String, usize)> = Vec::new();
    let mut out: Vec<Email> = Vec::new();
    for mut e in list.drain(..) {
        e.address = e.address.trim().to_string();
        if e.address.is_empty() {
            continue;
        }
        let key = e.address.to_lowercase();
        if let Some((_, i)) = seen.iter().find(|(k, _)| *k == key) {
            // Keep the first; take a display name or a better label from the repeat.
            let kept = &mut out[*i];
            if kept.display_name.is_empty() {
                kept.display_name = e.display_name;
            }
            if kept.label.is_empty() || kept.label == "other" {
                kept.label = e.label;
            }
            continue;
        }
        seen.push((key, out.len()));
        out.push(e);
    }
    *list = out;
    before - list.len()
}

/// Remove repeated phone numbers (the same number written differently counts as one) and empty
/// ones. Returns how many went.
pub fn dedupe_phones(list: &mut Vec<Labeled>) -> usize {
    let before = list.len();
    let mut out: Vec<Labeled> = Vec::new();
    let mut keys: Vec<String> = Vec::new();
    for mut p in list.drain(..) {
        p.value = p.value.trim().to_string();
        if p.value.is_empty() {
            continue;
        }
        let digits: String = p.value.chars().filter(|c| c.is_ascii_digit()).collect();
        let key = phone_key(&p.value).map(|k| {
            // An extension makes a different number.
            let ext = p.value.to_lowercase().split_once("ext").map(|(_, e)| e.chars().filter(|c| c.is_ascii_digit()).collect::<String>()).unwrap_or_default();
            format!("{k}/{ext}")
        });
        let key = key.unwrap_or(digits);
        if let Some(i) = keys.iter().position(|k| *k == key) {
            if out[i].label.is_empty() || out[i].label == "other" {
                out[i].label = p.label;
            }
            continue;
        }
        keys.push(key);
        out.push(p);
    }
    *list = out;
    before - list.len()
}

// --- duplicate contacts ------------------------------------------------------------------------

/// A contact's name for matching: lower-case words, sorted, so "Smith, Anna" and "anna smith"
/// match. None when the contact has no name to go by.
pub fn name_key(c: &Contact) -> Option<String> {
    let name = {
        let f = c.formatted_name();
        if !f.is_empty() {
            f
        } else if !c.name.display.trim().is_empty() {
            c.name.display.clone()
        } else {
            c.organizations.iter().map(|o| o.company.clone()).find(|s| !s.trim().is_empty())?
        }
    };
    let mut words: Vec<String> = name
        .to_lowercase()
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|w| !w.is_empty() && !["jr", "sr", "ii", "iii", "dr", "mr", "mrs", "ms"].contains(w))
        .map(String::from)
        .collect();
    words.sort();
    let key = words.join(" ");
    (key.chars().filter(|c| c.is_alphabetic()).count() >= 3).then_some(key)
}

/// Why contacts were put together.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DuplicateGroup {
    /// Indexes into the list given to [`duplicate_groups`].
    pub members: Vec<usize>,
    pub same_name: bool,
    pub same_number: bool,
}

/// Contacts that look like the same person: the same name, or a phone number in common.
/// `not_duplicates` holds id pairs the person said are different people (either order).
pub fn duplicate_groups(contacts: &[Contact], not_duplicates: &HashSet<(String, String)>) -> Vec<DuplicateGroup> {
    let n = contacts.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(p: &mut Vec<usize>, i: usize) -> usize {
        let mut r = i;
        while p[r] != r {
            r = p[r];
        }
        let mut j = i;
        while p[j] != r {
            let next = p[j];
            p[j] = r;
            j = next;
        }
        r
    }
    let mut by_name: HashMap<String, usize> = HashMap::new();
    let mut by_phone: HashMap<String, usize> = HashMap::new();
    let mut name_link: HashSet<(usize, usize)> = HashSet::new();
    let mut phone_link: HashSet<(usize, usize)> = HashSet::new();
    let apart = |a: usize, b: usize| {
        let (x, y) = (&contacts[a].id, &contacts[b].id);
        not_duplicates.contains(&(x.clone(), y.clone())) || not_duplicates.contains(&(y.clone(), x.clone()))
    };
    for i in 0..n {
        if let Some(k) = name_key(&contacts[i]) {
            match by_name.get(&k) {
                Some(&j) if !apart(i, j) => {
                    let (a, b) = (find(&mut parent, i), find(&mut parent, j));
                    parent[a] = b;
                    name_link.insert((j.min(i), j.max(i)));
                }
                Some(_) => {}
                None => {
                    by_name.insert(k, i);
                }
            }
        }
        for k in contacts[i].phone_keys() {
            match by_phone.get(&k) {
                Some(&j) if j != i && !apart(i, j) => {
                    let (a, b) = (find(&mut parent, i), find(&mut parent, j));
                    parent[a] = b;
                    phone_link.insert((j.min(i), j.max(i)));
                }
                Some(_) => {}
                None => {
                    by_phone.insert(k, i);
                }
            }
        }
    }
    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..n {
        let r = find(&mut parent, i);
        groups.entry(r).or_default().push(i);
    }
    let mut out: Vec<DuplicateGroup> = groups
        .into_values()
        .filter(|m| m.len() > 1)
        .map(|members| {
            let inside = |set: &HashSet<(usize, usize)>| set.iter().any(|(a, b)| members.contains(a) && members.contains(b));
            DuplicateGroup { same_name: inside(&name_link), same_number: inside(&phone_link), members }
        })
        .collect();
    for g in out.iter_mut() {
        g.members.sort();
    }
    out.sort_by_key(|g| g.members[0]);
    out
}

// --- comparing and merging ---------------------------------------------------------------------

const IGNORED: &[&str] = &["id", "created", "updated", "google"];

/// The contact as a flat map of fields ("name.given", "phones"...) → value. `normalize`
/// compares pictures by their content (their file names carry the contact's id) and sorts lists.
fn flatten(c: &Contact, normalize: bool) -> Vec<(String, Value)> {
    let Ok(Value::Object(map)) = serde_json::to_value(c) else { return Vec::new() };
    let mut out = Vec::new();
    for (k, v) in map {
        if IGNORED.contains(&k.as_str()) {
            continue;
        }
        match v {
            Value::Object(inner) if matches!(k.as_str(), "name" | "theme" | "outlook") => {
                for (ik, iv) in inner {
                    let path = format!("{k}.{ik}");
                    out.push((path.clone(), if normalize { norm(&path, iv) } else { iv }));
                }
            }
            v => out.push((k.clone(), if normalize { norm(&k, v) } else { v })),
        }
    }
    out
}

fn norm(path: &str, v: Value) -> Value {
    match v {
        Value::String(s) if path == "photo" || path == "theme.backgroundImage" => Value::String(s.split_once('-').map(|(_, h)| h.to_string()).unwrap_or(s)),
        Value::String(s) => Value::String(s.trim().to_string()),
        Value::Array(mut a) => {
            a.sort_by_key(|x| x.to_string());
            Value::Array(a)
        }
        v => v,
    }
}

fn default_values() -> HashMap<String, Value> {
    flatten(&Contact::new(), true).into_iter().collect()
}

fn is_empty(path: &str, v: &Value, defaults: &HashMap<String, Value>) -> bool {
    match v {
        Value::Null => true,
        Value::String(s) if s.trim().is_empty() => true,
        Value::Array(a) if a.is_empty() => true,
        Value::Object(o) if o.is_empty() => true,
        Value::Bool(false) => true,
        v => defaults.get(path) == Some(v),
    }
}

/// A field where the contacts disagree.
#[derive(Clone, Debug)]
pub struct Conflict {
    pub path: String,
    pub label: String,
    /// Each contact's value as text ("" when it has none).
    pub values: Vec<String>,
}

/// Whether the contacts hold exactly the same information (lists in any order).
pub fn identical(cs: &[Contact]) -> bool {
    let first: HashMap<String, Value> = match cs.first() {
        Some(c) => flatten(c, true).into_iter().collect(),
        None => return true,
    };
    cs.iter().skip(1).all(|c| flatten(c, true).into_iter().collect::<HashMap<_, _>>() == first)
}

/// The single-value fields where two or more contacts have different, non-empty values.
/// (Lists such as phones and emails never conflict: merging combines them.)
pub fn conflicts(cs: &[Contact]) -> Vec<Conflict> {
    let defaults = default_values();
    let flat: Vec<HashMap<String, Value>> = cs.iter().map(|c| flatten(c, true).into_iter().collect()).collect();
    let mut paths: Vec<String> = flat.iter().flat_map(|m| m.keys().cloned()).collect::<HashSet<_>>().into_iter().collect();
    paths.sort_by_key(|p| (field_order(p), p.clone()));
    let mut out = Vec::new();
    for p in paths {
        let vals: Vec<Option<&Value>> = flat.iter().map(|m| m.get(&p)).collect();
        if vals.iter().flatten().any(|v| v.is_array()) {
            continue;
        }
        let distinct: HashSet<String> = vals.iter().flatten().filter(|v| !is_empty(&p, v, &defaults)).map(|v| v.to_string().to_lowercase()).collect();
        if distinct.len() > 1 {
            let values = vals.iter().map(|v| v.filter(|v| !is_empty(&p, v, &defaults)).map(show_value).unwrap_or_default()).collect();
            out.push(Conflict { label: field_label(&p), path: p, values });
        }
    }
    out
}

fn show_value(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Bool(b) => if *b { "Yes".into() } else { "No".into() },
        other => other.to_string(),
    }
}

/// How complete a contact is: the number of fields with something in them.
pub fn completeness(c: &Contact) -> usize {
    let defaults = default_values();
    flatten(c, true)
        .into_iter()
        .map(|(p, v)| match &v {
            Value::Array(a) => a.len(),
            v if is_empty(&p, v, &defaults) => 0,
            _ => 1,
        })
        .sum()
}

/// The result of a merge.
pub struct Merged {
    pub contact: Contact,
    /// Which contact's picture the merged one uses (its file has to be carried over when that
    /// isn't the winner).
    pub photo_from: Option<usize>,
}

/// Merge contacts into one: start from `winner`; for each field in `picks` take that contact's
/// value; fill fields the winner leaves empty from the others; combine every list without
/// repeats. The result keeps the winner's id.
pub fn merge(cs: &[Contact], winner: usize, picks: &HashMap<String, usize>) -> Merged {
    let defaults = default_values();
    let real: Vec<HashMap<String, Value>> = cs.iter().map(|c| flatten(c, false).into_iter().collect()).collect();
    let normal: Vec<HashMap<String, Value>> = cs.iter().map(|c| flatten(c, true).into_iter().collect()).collect();
    let mut paths: Vec<String> = real.iter().flat_map(|m| m.keys().cloned()).collect::<HashSet<_>>().into_iter().collect();
    paths.sort();
    let mut top = Map::new();
    let mut photo_from = None;
    for p in &paths {
        let is_list = real.iter().any(|m| m.get(p).is_some_and(|v| v.is_array()));
        let value = if is_list {
            // Winner's items first, then the others' new ones.
            let mut items: Vec<Value> = Vec::new();
            let mut seen: HashSet<String> = HashSet::new();
            let order: Vec<usize> = std::iter::once(winner).chain((0..cs.len()).filter(|i| *i != winner)).collect();
            for i in order {
                if let Some(Value::Array(a)) = real[i].get(p) {
                    for item in a {
                        let key = item.to_string().to_lowercase();
                        if seen.insert(key) {
                            items.push(item.clone());
                        }
                    }
                }
            }
            Value::Array(items)
        } else {
            let filled = |i: usize| normal[i].get(p).is_some_and(|v| !is_empty(p, v, &defaults));
            let from = match picks.get(p) {
                Some(&i) if i < cs.len() => i,
                _ if filled(winner) => winner,
                _ => (0..cs.len()).find(|&i| filled(i)).unwrap_or(winner),
            };
            if p == "photo" && filled(from) {
                photo_from = Some(from);
            }
            real[from].get(p).cloned().unwrap_or(Value::Null)
        };
        match p.split_once('.') {
            Some((obj, key)) => {
                let entry = top.entry(obj.to_string()).or_insert_with(|| Value::Object(Map::new()));
                if let Value::Object(m) = entry {
                    m.insert(key.to_string(), value);
                }
            }
            None => {
                top.insert(p.clone(), value);
            }
        }
    }
    let mut contact: Contact = serde_json::from_value(Value::Object(top)).unwrap_or_else(|_| cs[winner].clone());
    let w = &cs[winner];
    contact.id = w.id.clone();
    contact.created = cs.iter().map(|c| c.created.clone()).filter(|s| !s.is_empty()).min().unwrap_or_else(|| w.created.clone());
    contact.updated = w.updated.clone();
    contact.google = w.google.clone().or_else(|| cs.iter().find_map(|c| c.google.clone()));
    contact.favorite = cs.iter().any(|c| c.favorite);
    dedupe_emails(&mut contact.emails);
    dedupe_phones(&mut contact.phones);
    let mut seen = HashSet::new();
    contact.labels.retain(|l| seen.insert(l.to_lowercase()));
    Merged { contact, photo_from }
}

/// Where a field sits in the merge screen (names first).
fn field_order(p: &str) -> u8 {
    if p.starts_with("name.") {
        0
    } else if matches!(p, "photo" | "kind") {
        1
    } else if p.starts_with("theme.") {
        5
    } else if p.starts_with("outlook.") {
        4
    } else if matches!(p, "notes") {
        3
    } else {
        2
    }
}

/// A field's name in plain words.
pub fn field_label(p: &str) -> String {
    let known: &[(&str, &str)] = &[
        ("name.prefix", "Prefix"),
        ("name.given", "First name"),
        ("name.middle", "Middle name"),
        ("name.family", "Last name"),
        ("name.suffix", "Suffix"),
        ("name.nickname", "Nickname"),
        ("name.maiden", "Maiden name"),
        ("name.phoneticGiven", "Phonetic first name"),
        ("name.phoneticMiddle", "Phonetic middle name"),
        ("name.phoneticFamily", "Phonetic last name"),
        ("name.initials", "Initials"),
        ("name.fileAs", "File as"),
        ("name.display", "Display name"),
        ("photo", "Picture"),
        ("kind", "Kind"),
        ("occupation", "Profession"),
        ("birthday", "Birthday"),
        ("anniversary", "Anniversary"),
        ("birthplace", "Birthplace"),
        ("deathdate", "Date of death"),
        ("deathplace", "Place of death"),
        ("gender", "Gender"),
        ("pronouns", "Pronouns"),
        ("timeZone", "Time zone"),
        ("geo", "Map location"),
        ("notes", "Notes"),
        ("favorite", "Favorite"),
    ];
    if let Some((_, l)) = known.iter().find(|(k, _)| *k == p) {
        return l.to_string();
    }
    let (group, key) = match p.split_once('.') {
        Some((g, k)) => (Some(g), k),
        None => (None, p),
    };
    let mut words = String::new();
    for (i, ch) in key.chars().enumerate() {
        if ch.is_uppercase() && i > 0 {
            words.push(' ');
            words.extend(ch.to_lowercase());
        } else if i == 0 {
            words.extend(ch.to_uppercase());
        } else {
            words.push(ch);
        }
    }
    match group {
        Some("theme") => format!("Theme: {}", words.to_lowercase()),
        Some("outlook") => format!("Outlook: {}", words.to_lowercase()),
        _ => words,
    }
}
