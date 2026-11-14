//! vCard 3.0 in and out (the format Outlook, Google Contacts and Apple Contacts all import and
//! export). Fields vCard has no standard name for travel as `X-CONTACTCRAFT-*` lines, so a
//! ContactCraft-to-ContactCraft round trip keeps everything; the theme travels as JSON.

use base64::Engine;

use crate::model::*;

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\n', "\\n").replace('\r', "").replace(',', "\\,").replace(';', "\\;")
}

fn unesc(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('n') | Some('N') => out.push('\n'),
                Some(o) => out.push(o),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Split on `sep` where it isn't escaped.
fn split_unescaped(s: &str, sep: char) -> Vec<String> {
    let mut parts = vec![String::new()];
    let mut prev_bs = false;
    for c in s.chars() {
        if c == sep && !prev_bs {
            parts.push(String::new());
        } else {
            parts.last_mut().unwrap().push(c);
        }
        prev_bs = c == '\\' && !prev_bs;
    }
    parts.into_iter().map(|p| unesc(&p)).collect()
}

/// Fold a line at 75 octets as the format asks.
fn fold(line: &str, out: &mut String) {
    let mut count = 0;
    for ch in line.chars() {
        let n = ch.len_utf8();
        if count + n > 75 {
            out.push_str("\r\n ");
            count = 1;
        }
        out.push(ch);
        count += n;
    }
    out.push_str("\r\n");
}

fn type_param(label: &str) -> String {
    let l = label.trim();
    if l.is_empty() {
        return String::new();
    }
    let std = matches!(l.to_lowercase().as_str(), "home" | "work" | "cell" | "fax" | "pager" | "voice" | "pref" | "internet" | "other" | "main" | "car" | "isdn" | "msg" | "video");
    if std {
        format!(";TYPE={}", l.to_uppercase())
    } else {
        format!(";TYPE=\"{}\"", l.replace('"', "'"))
    }
}

fn phone_type(label: &str) -> String {
    match label.to_lowercase().as_str() {
        "mobile" | "iphone" => ";TYPE=CELL".into(),
        "home fax" => ";TYPE=HOME,FAX".into(),
        "work fax" => ";TYPE=WORK,FAX".into(),
        "other fax" => ";TYPE=FAX".into(),
        "work mobile" => ";TYPE=WORK,CELL".into(),
        "work pager" => ";TYPE=WORK,PAGER".into(),
        "main" | "company main" => ";TYPE=MAIN".into(),
        _ => type_param(label),
    }
}

/// Contacts as one vCard 3.0 file. `photo` gives each contact's picture bytes, if any.
pub fn export(contacts: &[Contact], photo: impl Fn(&Contact) -> Option<Vec<u8>>) -> String {
    let mut out = String::new();
    for c in contacts {
        let mut l = |s: String| fold(&s, &mut out);
        l("BEGIN:VCARD".into());
        l("VERSION:3.0".into());
        let n = &c.name;
        l(format!("FN:{}", esc(&c.display_name())));
        l(format!("N:{};{};{};{};{}", esc(&n.family), esc(&n.given), esc(&n.middle), esc(&n.prefix), esc(&n.suffix)));
        if !n.nickname.is_empty() {
            l(format!("NICKNAME:{}", esc(&n.nickname)));
        }
        if !n.phonetic_given.is_empty() {
            l(format!("X-PHONETIC-FIRST-NAME:{}", esc(&n.phonetic_given)));
        }
        if !n.phonetic_middle.is_empty() {
            l(format!("X-PHONETIC-MIDDLE-NAME:{}", esc(&n.phonetic_middle)));
        }
        if !n.phonetic_family.is_empty() {
            l(format!("X-PHONETIC-LAST-NAME:{}", esc(&n.phonetic_family)));
        }
        if !n.maiden.is_empty() {
            l(format!("X-MAIDENNAME:{}", esc(&n.maiden)));
        }
        if !n.file_as.is_empty() {
            l(format!("SORT-STRING:{}", esc(&n.file_as)));
        }
        for o in &c.organizations {
            l(format!("ORG:{};{}", esc(&o.company), esc(&o.department)));
            if !o.title.is_empty() {
                l(format!("TITLE:{}", esc(&o.title)));
            }
            if !o.job_description.is_empty() {
                l(format!("ROLE:{}", esc(&o.job_description)));
            }
            if !o.phonetic_company.is_empty() {
                l(format!("X-PHONETIC-ORG:{}", esc(&o.phonetic_company)));
            }
        }
        for e in &c.emails {
            l(format!("EMAIL;TYPE=INTERNET{}:{}", type_param(&e.label).replacen(";TYPE=", ",", 1), esc(&e.address)));
        }
        for p in &c.phones {
            l(format!("TEL{}:{}", phone_type(&p.label), esc(&p.value)));
        }
        for a in &c.addresses {
            l(format!(
                "ADR{}:{};{};{};{};{};{};{}",
                type_param(&a.label),
                esc(&a.po_box),
                esc(&a.extended),
                esc(&a.street),
                esc(&a.city),
                esc(&a.region),
                esc(&a.postal_code),
                esc(&a.country)
            ));
        }
        for w in &c.websites {
            l(format!("URL{}:{}", type_param(&w.label), w.value));
        }
        for s in &c.social_profiles {
            l(format!("X-SOCIALPROFILE{}:{}", type_param(&s.label), esc(&s.value)));
        }
        for i in &c.im {
            l(format!("IMPP;X-SERVICE-TYPE={}{}:{}", esc(&i.protocol), type_param(&i.label), esc(&i.address)));
        }
        for s in &c.sip {
            l(format!("IMPP{}:sip:{}", type_param(&s.label), esc(s.value.trim_start_matches("sip:"))));
        }
        if !c.birthday.is_empty() {
            l(format!("BDAY:{}", c.birthday));
        }
        if !c.anniversary.is_empty() {
            l(format!("X-ANNIVERSARY:{}", c.anniversary));
        }
        for (i, e) in c.events.iter().enumerate() {
            l(format!("item{}.X-ABDATE:{}", i + 1, e.value));
            l(format!("item{}.X-ABLABEL:{}", i + 1, esc(&e.label)));
        }
        for (i, r) in c.relations.iter().enumerate() {
            l(format!("item{}.X-ABRELATEDNAMES:{}", i + 101, esc(&r.value)));
            l(format!("item{}.X-ABLABEL:{}", i + 101, esc(&r.label)));
        }
        if !c.gender.is_empty() {
            l(format!("X-GENDER:{}", esc(&c.gender)));
        }
        if !c.time_zone.is_empty() {
            l(format!("TZ:{}", esc(&c.time_zone)));
        }
        if !c.geo.is_empty() {
            l(format!("GEO:{}", c.geo.replace(',', ";")));
        }
        if !c.labels.is_empty() {
            l(format!("CATEGORIES:{}", c.labels.iter().map(|s| esc(s)).collect::<Vec<_>>().join(",")));
        }
        if !c.notes.is_empty() {
            l(format!("NOTE:{}", esc(&c.notes)));
        }
        for k in &c.public_keys {
            l(format!("KEY{}:{}", type_param(&k.label), esc(&k.value)));
        }
        if let Some(bytes) = photo(c) {
            let kind = if bytes.starts_with(&[0x89, b'P']) { "PNG" } else { "JPEG" };
            l(format!("PHOTO;ENCODING=b;TYPE={kind}:{}", base64::engine::general_purpose::STANDARD.encode(&bytes)));
        }
        // Everything else, ContactCraft's own way.
        let mut rest = c.clone();
        rest.photo.clear();
        rest.google = None;
        let full = serde_json::to_string(&rest).unwrap_or_default();
        l(format!("X-CONTACTCRAFT-JSON:{}", base64::engine::general_purpose::STANDARD.encode(full.as_bytes())));
        if !c.id.is_empty() {
            l(format!("UID:{}", c.id));
        }
        l(format!("REV:{}", c.updated));
        l("END:VCARD".into());
    }
    out
}

fn unfold(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for raw in text.split('\n') {
        let raw = raw.trim_end_matches('\r');
        if (raw.starts_with(' ') || raw.starts_with('\t')) && !lines.is_empty() {
            lines.last_mut().unwrap().push_str(&raw[1..]);
        } else {
            lines.push(raw.to_string());
        }
    }
    lines
}

struct Prop {
    group: String,
    name: String,
    types: Vec<String>,
    params: Vec<(String, String)>,
    value: String,
}

fn parse_line(line: &str) -> Option<Prop> {
    // The value starts at the first ':' outside quotes.
    let mut in_q = false;
    let mut split = None;
    for (i, ch) in line.char_indices() {
        match ch {
            '"' => in_q = !in_q,
            ':' if !in_q => {
                split = Some(i);
                break;
            }
            _ => {}
        }
    }
    let i = split?;
    let (head, value) = (&line[..i], &line[i + 1..]);
    let mut parts = head.split(';');
    let mut name = parts.next()?.to_string();
    let mut group = String::new();
    if let Some((g, n)) = name.clone().split_once('.') {
        group = g.to_lowercase();
        name = n.to_string();
    }
    let mut types = Vec::new();
    let mut params = Vec::new();
    for p in parts {
        let (k, v) = p.split_once('=').unwrap_or(("TYPE", p));
        let k = k.to_uppercase();
        for v in v.split(',') {
            let v = v.trim_matches('"').to_string();
            if k == "TYPE" {
                types.push(v.to_lowercase());
            } else {
                params.push((k.clone(), v));
            }
        }
    }
    Some(Prop { group, name: name.to_uppercase(), types, params, value: value.to_string() })
}

fn label_from(types: &[String], skip: &[&str]) -> String {
    types.iter().find(|t| !skip.contains(&t.as_str())).cloned().unwrap_or_else(|| "other".into())
}

/// Read every contact in a vCard file (versions 2.1, 3.0 and 4.0, as Outlook, Google and Apple
/// write them). Photos come back separately, as bytes, next to their contact.
pub fn import(text: &str) -> Vec<(Contact, Option<Vec<u8>>)> {
    let mut out = Vec::new();
    let mut cur: Option<(Contact, Option<Vec<u8>>, Vec<(String, String)>, Option<Contact>)> = None;
    for line in unfold(text) {
        let Some(p) = parse_line(&line) else { continue };
        if p.name == "BEGIN" && p.value.eq_ignore_ascii_case("VCARD") {
            cur = Some((Contact::new(), None, Vec::new(), None));
            continue;
        }
        let Some((c, photo, ab_labels, full)) = cur.as_mut() else { continue };
        let v = &p.value;
        match p.name.as_str() {
            "END" => {
                let (mut c, photo, labels, full) = cur.take().unwrap();
                // Apple's grouped dates and related names get their labels here.
                for (group, label) in labels {
                    if let Some(i) = group.strip_prefix("item").and_then(|n| n.parse::<usize>().ok()) {
                        if i > 100 {
                            if let Some(r) = c.relations.get_mut(i - 101) {
                                r.label = clean_ab_label(&label);
                            }
                        } else if let Some(e) = c.events.get_mut(i.saturating_sub(1)) {
                            e.label = clean_ab_label(&label);
                        }
                    }
                }
                if let Some(mut f) = full {
                    // Written by ContactCraft: the full record wins, keeping a fresh id.
                    f.id = String::new();
                    f.photo.clear();
                    c = f;
                }
                out.push((c, photo));
            }
            "FN" => c.name.display = unesc(v),
            "N" => {
                let f = split_unescaped(v, ';');
                let g = |i: usize| f.get(i).cloned().unwrap_or_default();
                c.name.family = g(0);
                c.name.given = g(1);
                c.name.middle = g(2);
                c.name.prefix = g(3);
                c.name.suffix = g(4);
            }
            "NICKNAME" => c.name.nickname = unesc(v),
            "X-PHONETIC-FIRST-NAME" => c.name.phonetic_given = unesc(v),
            "X-PHONETIC-MIDDLE-NAME" => c.name.phonetic_middle = unesc(v),
            "X-PHONETIC-LAST-NAME" => c.name.phonetic_family = unesc(v),
            "X-MAIDENNAME" => c.name.maiden = unesc(v),
            "SORT-STRING" => c.name.file_as = unesc(v),
            "ORG" => {
                let f = split_unescaped(v, ';');
                c.organizations.push(Organization { label: "work".into(), company: f.first().cloned().unwrap_or_default(), department: f.get(1).cloned().unwrap_or_default(), current: true, ..Default::default() });
            }
            "TITLE" | "ROLE" | "X-PHONETIC-ORG" => {
                if c.organizations.is_empty() {
                    c.organizations.push(Organization { label: "work".into(), current: true, ..Default::default() });
                }
                let o = c.organizations.last_mut().unwrap();
                match p.name.as_str() {
                    "TITLE" => o.title = unesc(v),
                    "ROLE" => o.job_description = unesc(v),
                    _ => o.phonetic_company = unesc(v),
                }
            }
            "EMAIL" => c.emails.push(Email { label: label_from(&p.types, &["internet", "pref", "x400"]), address: unesc(v), display_name: String::new() }),
            "TEL" => {
                let t = &p.types;
                let has = |s: &str| t.iter().any(|x| x == s);
                let label = if has("fax") {
                    if has("home") {
                        "home fax"
                    } else if has("work") {
                        "work fax"
                    } else {
                        "other fax"
                    }
                } else if has("cell") || has("mobile") {
                    if has("work") { "work mobile" } else { "mobile" }
                } else if has("pager") {
                    "pager"
                } else if has("main") {
                    "main"
                } else if has("home") {
                    "home"
                } else if has("work") {
                    "work"
                } else if has("car") {
                    "car"
                } else if has("isdn") {
                    "ISDN"
                } else if let Some(other) = t.iter().find(|x| !["voice", "pref"].contains(&x.as_str())) {
                    other.as_str()
                } else {
                    "other"
                };
                c.phones.push(Labeled::new(label, &unesc(v).trim_start_matches("tel:").to_string()));
            }
            "ADR" => {
                let f = split_unescaped(v, ';');
                let g = |i: usize| f.get(i).cloned().unwrap_or_default();
                c.addresses.push(Address {
                    label: label_from(&p.types, &["pref", "postal", "parcel", "dom", "intl"]),
                    po_box: g(0),
                    extended: g(1),
                    street: g(2),
                    city: g(3),
                    region: g(4),
                    postal_code: g(5),
                    country: g(6),
                    ..Default::default()
                });
            }
            "URL" => c.websites.push(Labeled::new(&label_from(&p.types, &["pref"]), &unesc(v))),
            "X-SOCIALPROFILE" => c.social_profiles.push(Labeled::new(&label_from(&p.types, &["pref"]), &unesc(v))),
            "IMPP" | "X-AIM" | "X-MSN" | "X-YAHOO" | "X-SKYPE" | "X-ICQ" | "X-JABBER" | "X-QQ" | "X-GOOGLE-TALK" => {
                let val = unesc(v);
                if let Some(sip) = val.strip_prefix("sip:") {
                    c.sip.push(Labeled::new(&label_from(&p.types, &["pref"]), sip));
                } else {
                    let protocol = p.params.iter().find(|(k, _)| k == "X-SERVICE-TYPE").map(|(_, v)| v.clone()).unwrap_or_else(|| {
                        if p.name == "IMPP" { val.split(':').next().unwrap_or("other").to_string() } else { p.name.trim_start_matches("X-").to_lowercase() }
                    });
                    let address = if p.name == "IMPP" { val.split_once(':').map(|(_, a)| a.to_string()).unwrap_or(val.clone()) } else { val };
                    c.im.push(Im { label: label_from(&p.types, &["pref"]), protocol, address });
                }
            }
            "BDAY" => c.birthday = normalize_date(v),
            "ANNIVERSARY" | "X-ANNIVERSARY" | "X-MS-ANNIVERSARY" => c.anniversary = normalize_date(v),
            "X-ABDATE" => c.events.push(Labeled::new("other", &normalize_date(v))),
            "X-ABRELATEDNAMES" => c.relations.push(Labeled::new("other", &unesc(v))),
            "X-ABLABEL" => ab_labels.push((p.group.clone(), unesc(v))),
            "X-SPOUSE" | "X-MS-SPOUSE" => c.relations.push(Labeled::new("spouse", &unesc(v))),
            "X-MS-MANAGER" | "X-MANAGER" => c.relations.push(Labeled::new("manager", &unesc(v))),
            "X-MS-ASSISTANT" | "X-ASSISTANT" => c.relations.push(Labeled::new("assistant", &unesc(v))),
            "X-GENDER" | "GENDER" => c.gender = unesc(v.split(';').next().unwrap_or("")),
            "TZ" => c.time_zone = unesc(v),
            "GEO" => c.geo = v.trim_start_matches("geo:").replace(';', ","),
            "CATEGORIES" => c.labels.extend(split_unescaped(v, ',').into_iter().filter(|s| !s.trim().is_empty())),
            "NOTE" => {
                if !c.notes.is_empty() {
                    c.notes.push('\n');
                }
                c.notes.push_str(&unesc(v));
            }
            "KEY" => c.public_keys.push(Labeled::new(&label_from(&p.types, &["pref"]), &unesc(v))),
            "LANG" => c.languages.push(unesc(v)),
            "KIND" | "X-ADDRESSBOOKSERVER-KIND" => c.kind = unesc(v).to_lowercase(),
            "BIRTHPLACE" => c.birthplace = unesc(v),
            "DEATHDATE" => c.deathdate = normalize_date(v),
            "DEATHPLACE" => c.deathplace = unesc(v),
            "HOBBY" => c.hobbies.push(unesc(v)),
            "INTEREST" => c.interests.push(unesc(v)),
            "EXPERTISE" => c.expertise.push(unesc(v)),
            "FBURL" => c.outlook.internet_free_busy = unesc(v),
            "CALURI" | "CALADRURI" => c.calendar_urls.push(Labeled::new(if p.name == "CALURI" { "home" } else { "availability" }, &unesc(v))),
            "PHOTO" => {
                let b64 = p.params.iter().any(|(k, v)| k == "ENCODING" && (v.eq_ignore_ascii_case("b") || v.eq_ignore_ascii_case("base64")));
                let data = if let Some(rest) = v.strip_prefix("data:") { rest.split_once(',').map(|(_, d)| d.to_string()) } else if b64 { Some(v.clone()) } else { None };
                if let Some(d) = data {
                    let clean: String = d.chars().filter(|c| !c.is_whitespace()).collect();
                    *photo = base64::engine::general_purpose::STANDARD.decode(clean).ok();
                }
            }
            "X-CONTACTCRAFT-JSON" => {
                if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(v.trim()) {
                    *full = serde_json::from_slice::<Contact>(&bytes).ok();
                }
            }
            n if n.starts_with("X-") && !matches!(n, "X-ABUID" | "X-ABSHOWAS") => {
                c.custom_fields.push(CustomField { label: n.trim_start_matches("X-").replace('-', " ").to_lowercase(), value: unesc(v), kind: "text".into() });
            }
            _ => {}
        }
    }
    // A file that ended early still gives what it had.
    if let Some((c, photo, _, _)) = cur {
        out.push((c, photo));
    }
    // A display name that is just the parts put together isn't kept separately.
    for (c, _) in out.iter_mut() {
        if c.name.display == c.formatted_name() {
            c.name.display.clear();
        }
    }
    out
}

fn clean_ab_label(l: &str) -> String {
    // Apple writes its own labels as _$!<Anniversary>!$_
    l.trim_start_matches("_$!<").trim_end_matches(">!$_").to_lowercase()
}

/// `19850412`, `1985-04-12`, `--0412`, `1985-04-12T00:00:00Z` → `1985-04-12` / `--04-12`.
pub fn normalize_date(v: &str) -> String {
    let v = v.trim();
    let date = v.split('T').next().unwrap_or(v);
    let digits: String = date.chars().filter(|c| c.is_ascii_digit()).collect();
    if date.starts_with("--") && digits.len() == 4 {
        return format!("--{}-{}", &digits[0..2], &digits[2..4]);
    }
    if digits.len() == 8 {
        return format!("{}-{}-{}", &digits[0..4], &digits[4..6], &digits[6..8]);
    }
    date.to_string()
}
