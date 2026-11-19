//! Contact files in CSV: Google Contacts' export ("Google CSV", old and new column names) and
//! Outlook's export ("Outlook CSV"), read by their column names. A column neither format names
//! is kept as a custom field, so nothing in the file is dropped.

use crate::model::*;
use crate::vcard::normalize_date;

/// Text from a file: UTF-8 (with or without a byte-order mark), UTF-16 with a mark, or the
/// Windows code page Outlook writes in.
pub fn decode_text(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
        let le = bytes[0] == 0xFF;
        let units: Vec<u16> = bytes[2..].chunks_exact(2).map(|c| if le { u16::from_le_bytes([c[0], c[1]]) } else { u16::from_be_bytes([c[0], c[1]]) }).collect();
        return String::from_utf16_lossy(&units);
    }
    let b = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    match std::str::from_utf8(b) {
        Ok(s) => s.to_string(),
        Err(_) => b.iter().map(|&x| cp1252(x)).collect(),
    }
}

fn cp1252(b: u8) -> char {
    const HIGH: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž', '\u{8f}', '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}', 'ž', 'Ÿ',
    ];
    if (0x80..0xA0).contains(&b) { HIGH[(b - 0x80) as usize] } else { b as char }
}

/// Rows of a CSV file (RFC 4180: quoted fields, doubled quotes, line breaks inside quotes).
pub fn parse_rows(text: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut in_q = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if in_q {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    field.push('"');
                    chars.next();
                } else {
                    in_q = false;
                }
            } else {
                field.push(c);
            }
            continue;
        }
        match c {
            '"' => in_q = true,
            ',' => row.push(std::mem::take(&mut field)),
            '\r' => {}
            '\n' => {
                row.push(std::mem::take(&mut field));
                if !(row.len() == 1 && row[0].is_empty()) {
                    rows.push(std::mem::take(&mut row));
                } else {
                    row.clear();
                }
            }
            _ => field.push(c),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}

/// Google writes several values in one cell as `a ::: b`.
fn multi(v: &str) -> Vec<String> {
    v.split(":::").map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
}

fn clean_label(l: &str) -> String {
    let l = l.trim().trim_start_matches("* ").trim();
    if l.is_empty() { "other".into() } else { l.to_lowercase() }
}

/// Which kind of CSV the header row says this is.
pub fn looks_like_outlook(header: &[String]) -> bool {
    header.iter().any(|h| h == "Business Phone" || h == "Mobile Phone" || h == "E-mail Address" || h == "Job Title")
}

/// Read a contacts CSV from Google Contacts or Outlook. Also returns, for each contact, the
/// address of its picture when the file names one (Google's `Photo` column).
pub fn import(text: &str) -> Vec<(Contact, Option<String>)> {
    let rows = parse_rows(text);
    let Some((header, body)) = rows.split_first() else { return Vec::new() };
    let header: Vec<String> = header.iter().map(|h| h.trim().trim_start_matches('\u{feff}').to_string()).collect();
    let outlook = looks_like_outlook(&header);
    body.iter()
        .filter(|r| r.iter().any(|v| !v.trim().is_empty()))
        .map(|r| {
            let cells: Vec<(&str, &str)> = header.iter().zip(r.iter()).map(|(h, v)| (h.as_str(), v.as_str())).collect();
            if outlook { (outlook_row(&cells), None) } else { google_row(&cells) }
        })
        .filter(|(c, _)| !c.is_blank())
        .collect()
}

/// `Phone 3 - Value` → ("Phone", 3, "Value").
fn numbered(h: &str) -> Option<(&str, usize, &str)> {
    let (left, right) = h.split_once(" - ")?;
    let (kind, n) = left.rsplit_once(' ')?;
    Some((kind, n.parse().ok()?, right))
}

fn slot<T: Default>(v: &mut Vec<T>, n: usize) -> &mut T {
    while v.len() < n {
        v.push(T::default());
    }
    &mut v[n - 1]
}

fn google_row(cells: &[(&str, &str)]) -> (Contact, Option<String>) {
    let mut c = Contact::new();
    let mut photo = None;
    let mut emails: Vec<(String, String)> = Vec::new();
    let mut phones: Vec<(String, String)> = Vec::new();
    let mut addrs: Vec<Address> = Vec::new();
    let mut webs: Vec<(String, String)> = Vec::new();
    let mut rels: Vec<(String, String)> = Vec::new();
    let mut events: Vec<(String, String)> = Vec::new();
    let mut customs: Vec<(String, String)> = Vec::new();
    let mut ims: Vec<(String, String, String)> = Vec::new();
    let mut orgs: Vec<Organization> = Vec::new();
    let mut extids: Vec<(String, String)> = Vec::new();
    let mut locs: Vec<(String, String)> = Vec::new();
    let mut cals: Vec<(String, String)> = Vec::new();
    let mut sips: Vec<(String, String)> = Vec::new();
    for &(h, v) in cells {
        if v.trim().is_empty() {
            continue;
        }
        let v = v.trim();
        match h {
            "Name" => c.name.display = v.into(),
            "First Name" | "Given Name" => c.name.given = v.into(),
            "Middle Name" | "Additional Name" => c.name.middle = v.into(),
            "Last Name" | "Family Name" => c.name.family = v.into(),
            "Phonetic First Name" | "Given Name Yomi" | "Yomi Name" => c.name.phonetic_given = v.into(),
            "Phonetic Middle Name" | "Additional Name Yomi" => c.name.phonetic_middle = v.into(),
            "Phonetic Last Name" | "Family Name Yomi" => c.name.phonetic_family = v.into(),
            "Name Prefix" => c.name.prefix = v.into(),
            "Name Suffix" => c.name.suffix = v.into(),
            "Nickname" => c.name.nickname = v.into(),
            "Short Name" | "Initials" => c.name.initials = v.into(),
            "Maiden Name" => c.name.maiden = v.into(),
            "File As" => c.name.file_as = v.into(),
            "Organization Name" => slot(&mut orgs, 1).company = v.into(),
            "Organization Title" => slot(&mut orgs, 1).title = v.into(),
            "Organization Department" => slot(&mut orgs, 1).department = v.into(),
            "Occupation" => c.occupation = v.into(),
            "Birthday" => c.birthday = normalize_date(v),
            "Gender" => c.gender = v.to_lowercase(),
            "Location" => locs.push(("other".into(), v.into())),
            "Billing Information" => c.outlook.billing_information = v.into(),
            "Directory Server" => c.outlook.directory_server = v.into(),
            "Mileage" => c.outlook.mileage = v.into(),
            "Priority" => c.outlook.priority = v.to_lowercase(),
            "Sensitivity" => c.outlook.sensitivity = v.to_lowercase(),
            "Subject" => c.outlook.subject = v.into(),
            "Hobby" => c.hobbies.extend(multi(v)),
            "Language" => c.languages.extend(multi(v)),
            "Notes" => c.notes = v.into(),
            "Photo" => photo = Some(v.to_string()),
            "Labels" | "Group Membership" => {
                for l in multi(v) {
                    let l = l.trim_start_matches("* ").to_string();
                    if !matches!(l.as_str(), "myContacts" | "starred" | "My Contacts") {
                        c.labels.push(l);
                    } else if l == "starred" {
                        c.favorite = true;
                    }
                }
            }
            _ => {
                if let Some((kind, n, part)) = numbered(h) {
                    let is_label = part == "Label" || part == "Type";
                    match kind {
                        "E-mail" => {
                            let e = slot(&mut emails, n);
                            if is_label { e.0 = clean_label(v) } else { e.1 = v.into() }
                        }
                        "Phone" => {
                            let e = slot(&mut phones, n);
                            if is_label { e.0 = clean_label(v) } else { e.1 = v.into() }
                        }
                        "Website" => {
                            let e = slot(&mut webs, n);
                            if is_label { e.0 = clean_label(v) } else { e.1 = v.into() }
                        }
                        "Relation" => {
                            let e = slot(&mut rels, n);
                            if is_label { e.0 = clean_label(v) } else { e.1 = v.into() }
                        }
                        "Event" => {
                            let e = slot(&mut events, n);
                            if is_label { e.0 = clean_label(v) } else { e.1 = normalize_date(v) }
                        }
                        "Custom Field" => {
                            let e = slot(&mut customs, n);
                            if is_label { e.0 = v.into() } else { e.1 = v.into() }
                        }
                        "External ID" => {
                            let e = slot(&mut extids, n);
                            if is_label { e.0 = clean_label(v) } else { e.1 = v.into() }
                        }
                        "Location" => {
                            let e = slot(&mut locs, n);
                            if is_label { e.0 = clean_label(v) } else { e.1 = v.into() }
                        }
                        "Calendar Link" | "Calendar" => {
                            let e = slot(&mut cals, n);
                            if is_label { e.0 = clean_label(v) } else { e.1 = v.into() }
                        }
                        "SIP" | "Sip Address" => {
                            let e = slot(&mut sips, n);
                            if is_label { e.0 = clean_label(v) } else { e.1 = v.into() }
                        }
                        "IM" => {
                            let e = slot(&mut ims, n);
                            match part {
                                "Label" | "Type" => e.0 = clean_label(v),
                                "Service" => e.1 = v.into(),
                                _ => e.2 = v.into(),
                            }
                        }
                        "Organization" => {
                            let o = slot(&mut orgs, n);
                            match part {
                                "Type" | "Label" => o.label = clean_label(v),
                                "Name" => o.company = v.into(),
                                "Yomi Name" => o.phonetic_company = v.into(),
                                "Title" => o.title = v.into(),
                                "Department" => o.department = v.into(),
                                "Symbol" => o.symbol = v.into(),
                                "Location" => o.office = v.into(),
                                "Job Description" => o.job_description = v.into(),
                                _ => {}
                            }
                        }
                        "Address" => {
                            let a = slot(&mut addrs, n);
                            match part {
                                "Label" | "Type" => a.label = clean_label(v),
                                "Street" => a.street = v.into(),
                                "City" => a.city = v.into(),
                                "PO Box" => a.po_box = v.into(),
                                "Region" => a.region = v.into(),
                                "Postal Code" => a.postal_code = v.into(),
                                "Country" => a.country = v.into(),
                                "Extended Address" => a.extended = v.into(),
                                "Formatted" => {
                                    if a.street.is_empty() && a.city.is_empty() {
                                        a.street = v.into();
                                    }
                                }
                                _ => {}
                            }
                        }
                        _ => c.custom_fields.push(CustomField { label: h.into(), value: v.into(), kind: "text".into() }),
                    }
                } else {
                    c.custom_fields.push(CustomField { label: h.into(), value: v.into(), kind: "text".into() });
                }
            }
        }
    }
    // One cell can hold several values ("home ::: a@x.com ::: b@x.com").
    let spread = |list: Vec<(String, String)>| -> Vec<Labeled> {
        list.into_iter().flat_map(|(l, v)| multi(&v).into_iter().map(move |x| Labeled::new(if l.is_empty() { "other" } else { &l }, &x))).collect()
    };
    c.emails = spread(emails).into_iter().map(|l| Email { label: l.label, address: l.value, display_name: String::new() }).collect();
    c.phones = spread(phones);
    c.websites = spread(webs);
    c.relations = spread(rels);
    c.events = events.into_iter().filter(|e| !e.1.is_empty()).map(|(l, v)| Labeled::new(&l, &v)).collect();
    if let Some(i) = c.events.iter().position(|e| e.label == "anniversary") {
        c.anniversary = c.events.remove(i).value;
    }
    c.external_ids = spread(extids);
    c.locations.extend(spread(locs));
    c.calendar_urls = spread(cals);
    c.sip = spread(sips);
    c.custom_fields.extend(customs.into_iter().filter(|e| !e.1.is_empty()).map(|(l, v)| CustomField { label: l, value: v, kind: "text".into() }));
    c.im = ims.into_iter().filter(|e| !e.2.is_empty()).map(|(l, p, a)| Im { label: l, protocol: p, address: a }).collect();
    c.addresses = addrs.into_iter().filter(|a| !a.is_empty()).collect();
    c.organizations = orgs
        .into_iter()
        .filter(|o| !(o.company.is_empty() && o.title.is_empty() && o.department.is_empty()))
        .map(|mut o| {
            if o.label.is_empty() {
                o.label = "work".into();
            }
            o.current = true;
            o
        })
        .collect();
    if c.name.display == c.formatted_name() {
        c.name.display.clear();
    }
    (c, photo)
}

fn outlook_row(cells: &[(&str, &str)]) -> Contact {
    let mut c = Contact::new();
    let mut org = Organization { label: "work".into(), current: true, ..Default::default() };
    let mut addr: [Address; 3] = [
        Address { label: "work".into(), ..Default::default() },
        Address { label: "home".into(), ..Default::default() },
        Address { label: "other".into(), ..Default::default() },
    ];
    let mut emails: [Email; 3] = Default::default();
    for &(h, v) in cells {
        let v = v.trim();
        if v.is_empty() {
            continue;
        }
        let phone = |c: &mut Contact, l: &str| c.phones.push(Labeled::new(l, v));
        // Business / Home / Other addresses.
        let addr_part = |prefix: &str| -> Option<&str> { h.strip_prefix(prefix) };
        let mut handled = false;
        for (i, prefix) in ["Business ", "Home ", "Other "].iter().enumerate() {
            if let Some(part) = addr_part(prefix) {
                let a = &mut addr[i];
                handled = true;
                match part {
                    "Street" => a.street = v.into(),
                    "Street 2" | "Street 3" => {
                        if !a.extended.is_empty() {
                            a.extended.push('\n');
                        }
                        a.extended.push_str(v);
                    }
                    "City" => a.city = v.into(),
                    "State" => a.region = v.into(),
                    "Postal Code" => a.postal_code = v.into(),
                    "Country/Region" | "Country" => a.country = v.into(),
                    "Address PO Box" => a.po_box = v.into(),
                    _ => handled = false,
                }
            }
        }
        if handled {
            continue;
        }
        match h {
            "Title" => c.name.prefix = v.into(),
            "First Name" => c.name.given = v.into(),
            "Middle Name" => c.name.middle = v.into(),
            "Last Name" => c.name.family = v.into(),
            "Suffix" => c.name.suffix = v.into(),
            "Nickname" => c.name.nickname = v.into(),
            "Initials" => c.name.initials = v.into(),
            "Given Yomi" => c.name.phonetic_given = v.into(),
            "Surname Yomi" => c.name.phonetic_family = v.into(),
            "Company" => org.company = v.into(),
            "Company Yomi" => org.phonetic_company = v.into(),
            "Department" => org.department = v.into(),
            "Job Title" => org.title = v.into(),
            "Office Location" => org.office = v.into(),
            "Profession" => c.occupation = v.into(),
            "Assistant's Phone" => phone(&mut c, "assistant"),
            "Business Fax" => phone(&mut c, "work fax"),
            "Business Phone" | "Business Phone 2" => phone(&mut c, "work"),
            "Callback" => phone(&mut c, "callback"),
            "Car Phone" => phone(&mut c, "car"),
            "Company Main Phone" => phone(&mut c, "company main"),
            "Home Fax" => phone(&mut c, "home fax"),
            "Home Phone" | "Home Phone 2" => phone(&mut c, "home"),
            "ISDN" => phone(&mut c, "ISDN"),
            "Mobile Phone" => phone(&mut c, "mobile"),
            "Other Fax" => phone(&mut c, "other fax"),
            "Other Phone" => phone(&mut c, "other"),
            "Pager" => phone(&mut c, "pager"),
            "Primary Phone" => phone(&mut c, "primary"),
            "Radio Phone" => phone(&mut c, "radio"),
            "TTY/TDD Phone" => phone(&mut c, "TTY/TDD"),
            "Telex" => phone(&mut c, "telex"),
            "E-mail Address" => emails[0].address = v.into(),
            "E-mail Display Name" => emails[0].display_name = v.into(),
            "E-mail 2 Address" => emails[1].address = v.into(),
            "E-mail 2 Display Name" => emails[1].display_name = v.into(),
            "E-mail 3 Address" => emails[2].address = v.into(),
            "E-mail 3 Display Name" => emails[2].display_name = v.into(),
            "E-mail Type" | "E-mail 2 Type" | "E-mail 3 Type" => {}
            "Account" => c.outlook.account = v.into(),
            "Anniversary" => c.anniversary = outlook_date(v),
            "Birthday" => c.birthday = outlook_date(v),
            "Assistant's Name" => c.relations.push(Labeled::new("assistant", v)),
            "Manager's Name" => c.relations.push(Labeled::new("manager", v)),
            "Spouse" => c.relations.push(Labeled::new("spouse", v)),
            "Children" => {
                for child in v.split([',', ';']).map(str::trim).filter(|s| !s.is_empty()) {
                    c.relations.push(Labeled::new("child", child));
                }
            }
            "Referred By" => c.relations.push(Labeled::new("referred by", v)),
            "Billing Information" => c.outlook.billing_information = v.into(),
            "Categories" => c.labels.extend(v.split(';').map(str::trim).filter(|s| !s.is_empty()).map(String::from)),
            "Directory Server" => c.outlook.directory_server = v.into(),
            "Gender" => c.gender = if v.eq_ignore_ascii_case("unspecified") { String::new() } else { v.to_lowercase() },
            "Government ID Number" => c.outlook.government_id = v.into(),
            "Hobby" => c.hobbies.extend(v.split([',', ';']).map(str::trim).filter(|s| !s.is_empty()).map(String::from)),
            "Internet Free Busy" => c.outlook.internet_free_busy = v.into(),
            "Keywords" => c.outlook.keywords = v.into(),
            "Language" => c.languages.push(v.into()),
            "Location" => c.locations.push(Labeled::new("other", v)),
            "Mileage" => c.outlook.mileage = v.into(),
            "Notes" => c.notes = v.into(),
            "Organizational ID Number" => c.outlook.organizational_id = v.into(),
            "Priority" => c.outlook.priority = v.to_lowercase(),
            "Private" => c.outlook.private = v.eq_ignore_ascii_case("true"),
            "Sensitivity" => c.outlook.sensitivity = v.to_lowercase(),
            "User 1" => c.outlook.user1 = v.into(),
            "User 2" => c.outlook.user2 = v.into(),
            "User 3" => c.outlook.user3 = v.into(),
            "User 4" => c.outlook.user4 = v.into(),
            "Web Page" => c.websites.push(Labeled::new("home page", v)),
            "Personal Web Page" => c.websites.push(Labeled::new("home page", v)),
            "Business Web Page" => c.websites.push(Labeled::new("work", v)),
            "FTP Site" => c.outlook.ftp_site = v.into(),
            "Computer Network Name" => c.outlook.computer_network_name = v.into(),
            "Customer ID" => c.outlook.customer_id = v.into(),
            "Journal" => c.outlook.journal = v.eq_ignore_ascii_case("true"),
            _ => c.custom_fields.push(CustomField { label: h.into(), value: v.into(), kind: "text".into() }),
        }
    }
    if !(org.company.is_empty() && org.title.is_empty() && org.department.is_empty() && org.office.is_empty()) {
        c.organizations.push(org);
    }
    for (i, e) in emails.into_iter().enumerate() {
        if !e.address.is_empty() {
            c.emails.push(Email { label: ["work", "home", "other"][i].to_string(), ..e });
        }
    }
    for a in addr {
        if !a.is_empty() {
            c.addresses.push(a);
        }
    }
    c
}

/// Outlook writes dates as the computer's short date (`4/12/1985`), and `0/0/00` for none.
fn outlook_date(v: &str) -> String {
    let parts: Vec<&str> = v.split(['/', '-', '.']).collect();
    if parts.len() == 3 {
        let n: Vec<u32> = parts.iter().filter_map(|p| p.trim().parse().ok()).collect();
        if n.len() == 3 {
            if n[0] == 0 || n[1] == 0 {
                return String::new();
            }
            let (y, m, d) = if n[0] > 31 { (n[0], n[1], n[2]) } else { (n[2], n[0], n[1]) };
            let y = if y < 100 { if y > 30 { 1900 + y } else { 2000 + y } } else { y };
            return format!("{y:04}-{m:02}-{d:02}");
        }
    }
    normalize_date(v)
}

/// Contacts as Google CSV, the file Google Contacts imports.
pub fn export_google(contacts: &[Contact]) -> String {
    let max = |f: &dyn Fn(&Contact) -> usize| contacts.iter().map(f).max().unwrap_or(0).max(1);
    let (ne, np, na, nw, nr, nv, nc, ni) = (
        max(&|c| c.emails.len()),
        max(&|c| c.phones.len()),
        max(&|c| c.addresses.len()),
        max(&|c| c.websites.len()),
        max(&|c| c.relations.len()),
        max(&|c| c.events.len() + usize::from(!c.anniversary.is_empty())),
        max(&|c| c.custom_fields.len()),
        max(&|c| c.im.len()),
    );
    let mut head: Vec<String> = [
        "First Name", "Middle Name", "Last Name", "Phonetic First Name", "Phonetic Middle Name", "Phonetic Last Name", "Name Prefix", "Name Suffix", "Nickname", "File As",
        "Organization Name", "Organization Title", "Organization Department", "Birthday", "Notes", "Labels",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let num = |head: &mut Vec<String>, kind: &str, n: usize, parts: &[&str]| {
        for i in 1..=n {
            for p in parts {
                head.push(format!("{kind} {i} - {p}"));
            }
        }
    };
    num(&mut head, "E-mail", ne, &["Label", "Value"]);
    num(&mut head, "Phone", np, &["Label", "Value"]);
    num(&mut head, "Address", na, &["Label", "Street", "City", "PO Box", "Region", "Postal Code", "Country", "Extended Address"]);
    num(&mut head, "Website", nw, &["Label", "Value"]);
    num(&mut head, "Relation", nr, &["Label", "Value"]);
    num(&mut head, "Event", nv, &["Label", "Value"]);
    num(&mut head, "IM", ni, &["Label", "Service", "Value"]);
    num(&mut head, "Custom Field", nc, &["Label", "Value"]);
    let mut out = String::new();
    let line = |cells: &[String], out: &mut String| {
        let q: Vec<String> = cells.iter().map(|c| if c.contains([',', '"', '\n']) { format!("\"{}\"", c.replace('"', "\"\"")) } else { c.clone() }).collect();
        out.push_str(&q.join(","));
        out.push_str("\r\n");
    };
    line(&head, &mut out);
    for c in contacts {
        let n = &c.name;
        let o = c.organizations.first().cloned().unwrap_or_default();
        let mut labels: Vec<String> = c.labels.clone();
        if c.favorite {
            labels.push("* starred".into());
        }
        labels.insert(0, "* myContacts".into());
        let mut row: Vec<String> = vec![
            n.given.clone(),
            n.middle.clone(),
            n.family.clone(),
            n.phonetic_given.clone(),
            n.phonetic_middle.clone(),
            n.phonetic_family.clone(),
            n.prefix.clone(),
            n.suffix.clone(),
            n.nickname.clone(),
            n.file_as.clone(),
            o.company,
            o.title,
            o.department,
            c.birthday.clone(),
            c.notes.clone(),
            labels.join(" ::: "),
        ];
        let pad2 = |row: &mut Vec<String>, items: Vec<(String, String)>, n: usize| {
            for i in 0..n {
                let (l, v) = items.get(i).cloned().unwrap_or_default();
                row.push(l);
                row.push(v);
            }
        };
        pad2(&mut row, c.emails.iter().map(|e| (e.label.clone(), e.address.clone())).collect(), ne);
        pad2(&mut row, c.phones.iter().map(|p| (p.label.clone(), p.value.clone())).collect(), np);
        for i in 0..na {
            let a = c.addresses.get(i).cloned().unwrap_or_default();
            row.extend([a.label, a.street, a.city, a.po_box, a.region, a.postal_code, a.country, a.extended]);
        }
        pad2(&mut row, c.websites.iter().map(|p| (p.label.clone(), p.value.clone())).collect(), nw);
        pad2(&mut row, c.relations.iter().map(|p| (p.label.clone(), p.value.clone())).collect(), nr);
        let mut ev: Vec<(String, String)> = c.events.iter().map(|p| (p.label.clone(), p.value.clone())).collect();
        if !c.anniversary.is_empty() {
            ev.insert(0, ("Anniversary".into(), c.anniversary.clone()));
        }
        pad2(&mut row, ev, nv);
        for i in 0..ni {
            let m = c.im.get(i).cloned().unwrap_or_default();
            row.extend([m.label, m.protocol, m.address]);
        }
        pad2(&mut row, c.custom_fields.iter().map(|p| (p.label.clone(), p.value.clone())).collect(), nc);
        line(&row, &mut out);
    }
    out
}
