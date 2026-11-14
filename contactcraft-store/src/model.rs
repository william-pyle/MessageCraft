//! One contact, with every kind of information the major contact formats keep: Google Contacts
//! (People API), Outlook / Exchange, vCard 3 and 4 (with the RFC 6474 and 6715 additions) and
//! Apple Contacts. Anything a newer version of ContactCraft (or another Craft app) adds that this
//! version doesn't know is kept as it is in `extra`, so nothing is ever lost on a round trip.
//!
//! Every list (phones, emails, addresses...) is a list of labelled values, so a contact can have
//! as many as it needs, with any label: the standard ones (`home`, `work`, `mobile`...) or the
//! person's own.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A value with a label: "mobile" +1 555..., "home" https://...
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Labeled {
    pub label: String,
    pub value: String,
}

impl Labeled {
    pub fn new(label: &str, value: &str) -> Self {
        Labeled { label: label.into(), value: value.into() }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Name {
    pub prefix: String,
    pub given: String,
    pub middle: String,
    pub family: String,
    pub suffix: String,
    pub nickname: String,
    pub maiden: String,
    /// Phonetic (Outlook "Yomi") spellings.
    pub phonetic_given: String,
    pub phonetic_middle: String,
    pub phonetic_family: String,
    /// Outlook's initials field.
    pub initials: String,
    /// How the contact files and sorts ("Smith, Anna"). Empty: family, given.
    pub file_as: String,
    /// The name to show, when it isn't simply the parts put together.
    pub display: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Email {
    pub label: String,
    pub address: String,
    /// Outlook's "Display as".
    pub display_name: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Address {
    pub label: String,
    pub street: String,
    /// Apartment, suite, floor.
    pub extended: String,
    pub po_box: String,
    pub city: String,
    pub region: String,
    pub postal_code: String,
    pub country: String,
    /// ISO 3166-1 code ("US").
    pub country_code: String,
    /// Outlook: this is the mailing address.
    pub mailing: bool,
}

impl Address {
    pub fn is_empty(&self) -> bool {
        [&self.street, &self.extended, &self.po_box, &self.city, &self.region, &self.postal_code, &self.country].iter().all(|s| s.trim().is_empty())
    }

    /// The address as lines, the way an envelope reads.
    pub fn lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        for s in [&self.street, &self.extended] {
            for l in s.lines() {
                if !l.trim().is_empty() {
                    out.push(l.trim().to_string());
                }
            }
        }
        if !self.po_box.trim().is_empty() {
            out.push(format!("PO Box {}", self.po_box.trim()));
        }
        let mut city = self.city.trim().to_string();
        if !self.region.trim().is_empty() {
            if !city.is_empty() {
                city.push_str(", ");
            }
            city.push_str(self.region.trim());
        }
        if !self.postal_code.trim().is_empty() {
            if !city.is_empty() {
                city.push(' ');
            }
            city.push_str(self.postal_code.trim());
        }
        if !city.is_empty() {
            out.push(city);
        }
        if !self.country.trim().is_empty() {
            out.push(self.country.trim().to_string());
        }
        out
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Organization {
    pub label: String,
    pub company: String,
    pub phonetic_company: String,
    pub department: String,
    pub title: String,
    pub job_description: String,
    /// Stock symbol.
    pub symbol: String,
    /// Outlook's "Office" location.
    pub office: String,
    pub cost_center: String,
    pub domain: String,
    pub start_date: String,
    pub end_date: String,
    pub current: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Im {
    pub label: String,
    /// aim, msn, yahoo, skype, qq, googleTalk, icq, jabber, netMeeting, teams, signal, whatsapp...
    pub protocol: String,
    pub address: String,
}

/// A field the person made up, with what kind of value it holds.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct CustomField {
    pub label: String,
    pub value: String,
    /// text, number, date, url, email, phone or yesno.
    pub kind: String,
}

/// The fields Outlook keeps that the other formats don't have a name for.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct OutlookFields {
    pub billing_information: String,
    pub directory_server: String,
    pub mileage: String,
    /// low, normal or high.
    pub priority: String,
    /// normal, personal, private or confidential.
    pub sensitivity: String,
    pub private: bool,
    pub subject: String,
    pub keywords: String,
    pub user1: String,
    pub user2: String,
    pub user3: String,
    pub user4: String,
    pub government_id: String,
    pub account: String,
    pub customer_id: String,
    pub organizational_id: String,
    pub computer_network_name: String,
    pub ftp_site: String,
    pub internet_free_busy: String,
    pub journal: bool,
    pub follow_up_flag: String,
    pub follow_up_due: String,
    pub follow_up_done: bool,
}

/// How a contact looks across the Craft apps: the banner, the page behind their card and their
/// conversation (colour or picture), the font, size and colour of their text, an emoji, the
/// colours of their messages in MessageCraft, and their sounds.
///
/// Colours are `#RRGGBB`; an empty colour means "the app's own". Pictures and font files live in
/// the store's `backgrounds` and `fonts` folders (see [`crate::Store::theme_file`]).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct ContactTheme {
    /// Main colour. Empty: a colour picked from the name.
    pub accent: String,
    /// Second colour for gradients and patterns.
    pub accent2: String,
    /// The banner: solid, gradient, diagonal, stripes, dots or glow.
    pub style: String,
    /// Text on the banner. Empty: white or black, whichever reads better.
    pub text: String,
    /// circle, rounded or square.
    pub avatar_shape: String,
    /// One emoji shown with the contact.
    pub emoji: String,

    /// The font family for this person's text ("Georgia"); empty for the app's own.
    pub font: String,
    /// The font's file: a Windows font file name (`georgia.ttf`) or one copied into the store's
    /// `fonts` folder.
    pub font_file: String,
    /// Text size in points; 0 for the app's own.
    pub text_size: f32,
    pub bold: bool,
    pub italic: bool,
    /// Text colour on the background.
    pub text_color: String,
    /// Colour for links, numbers and addresses.
    pub link_color: String,

    /// The background behind their card and their conversation.
    pub background: String,
    /// A picture in the store's `backgrounds` folder; empty for none.
    pub background_image: String,
    /// cover, contain, tile, center or stretch.
    pub background_fit: String,
    /// How strongly the picture shows, 0–100.
    pub background_opacity: u8,
    /// Blur the picture a little so text stays readable.
    pub background_blur: bool,

    /// Their messages and ours in a conversation (MessageCraft), and the text on each.
    pub bubble_in: String,
    pub bubble_in_text: String,
    pub bubble_out: String,
    pub bubble_out_text: String,
    /// Bubble corner roundness in pixels; 0 for the app's own.
    pub bubble_radius: u8,

    /// A sound name or file for this person's messages and calls.
    pub ringtone: String,
    pub text_tone: String,
}

impl Default for ContactTheme {
    fn default() -> Self {
        ContactTheme {
            accent: String::new(),
            accent2: String::new(),
            style: "gradient".into(),
            text: String::new(),
            avatar_shape: "circle".into(),
            emoji: String::new(),
            font: String::new(),
            font_file: String::new(),
            text_size: 0.0,
            bold: false,
            italic: false,
            text_color: String::new(),
            link_color: String::new(),
            background: String::new(),
            background_image: String::new(),
            background_fit: "cover".into(),
            background_opacity: 100,
            background_blur: false,
            bubble_in: String::new(),
            bubble_in_text: String::new(),
            bubble_out: String::new(),
            bubble_out_text: String::new(),
            bubble_radius: 0,
            ringtone: String::new(),
            text_tone: String::new(),
        }
    }
}

impl ContactTheme {
    /// Whether the person changed anything from the plain look.
    pub fn is_default(&self) -> bool {
        *self == ContactTheme::default()
    }
}

/// Where the contact lives in Google Contacts, once synced.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct GoogleLink {
    pub account: String,
    /// people/c123...
    pub resource_name: String,
    pub etag: String,
    pub update_time: String,
    pub photo_url: String,
    /// Contact groups (labels) it belongs to, as Google names them.
    pub groups: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Contact {
    /// Stable id (32 hex digits). Empty until the contact is first saved.
    pub id: String,
    /// When it was made and last changed (RFC 3339, UTC).
    pub created: String,
    pub updated: String,
    /// individual, organization, group or location (vCard KIND).
    pub kind: String,

    pub name: Name,
    pub organizations: Vec<Organization>,
    /// Outlook "Profession", Google "Occupation".
    pub occupation: String,
    pub emails: Vec<Email>,
    pub phones: Vec<Labeled>,
    pub addresses: Vec<Address>,
    /// Home page, work, blog, profile, FTP...
    pub websites: Vec<Labeled>,
    pub social_profiles: Vec<Labeled>,
    pub im: Vec<Im>,
    pub sip: Vec<Labeled>,

    /// `YYYY-MM-DD`, or `--MM-DD` when the year isn't known.
    pub birthday: String,
    pub anniversary: String,
    /// Other dates ("first met", "graduation"...), value in the same form.
    pub events: Vec<Labeled>,
    pub birthplace: String,
    pub deathdate: String,
    pub deathplace: String,

    /// spouse, partner, child, mother, father, parent, brother, sister, friend, relative,
    /// manager, assistant, referred-by, domestic-partner...: label and the person's name.
    pub relations: Vec<Labeled>,

    pub gender: String,
    pub pronouns: String,
    /// Languages they speak, preferred first (`en-US`).
    pub languages: Vec<String>,
    pub time_zone: String,
    /// `lat,lon`.
    pub geo: String,
    pub hobbies: Vec<String>,
    pub interests: Vec<String>,
    pub skills: Vec<String>,
    pub expertise: Vec<String>,
    /// Desks, buildings, floors: label and value.
    pub locations: Vec<Labeled>,
    /// account, customer, network, organization, government...: label and id.
    pub external_ids: Vec<Labeled>,
    pub calendar_urls: Vec<Labeled>,
    /// PGP / S/MIME keys or their addresses.
    pub public_keys: Vec<Labeled>,
    pub custom_fields: Vec<CustomField>,

    /// Labels (Google contact groups, Outlook categories).
    pub labels: Vec<String>,
    pub notes: String,
    pub favorite: bool,

    /// The picture's file name inside the store's `photos` folder; empty for none.
    pub photo: String,
    pub theme: ContactTheme,
    pub outlook: OutlookFields,
    pub google: Option<GoogleLink>,

    /// Fields this version doesn't know (written by a newer app): kept as they are.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Contact {
    pub fn new() -> Self {
        Contact { kind: "individual".into(), ..Default::default() }
    }

    /// The parts of the name put together: "Dr. Anna M. Smith Jr."
    pub fn formatted_name(&self) -> String {
        let n = &self.name;
        [&n.prefix, &n.given, &n.middle, &n.family, &n.suffix].iter().map(|s| s.trim()).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ")
    }

    /// The name to show everywhere: the display name, else the name, the nickname, the company,
    /// the first email address or the first phone number.
    pub fn display_name(&self) -> String {
        if !self.name.display.trim().is_empty() {
            return self.name.display.trim().to_string();
        }
        let f = self.formatted_name();
        if !f.is_empty() {
            return f;
        }
        if !self.name.nickname.trim().is_empty() {
            return self.name.nickname.trim().to_string();
        }
        if let Some(o) = self.organizations.iter().find(|o| !o.company.trim().is_empty()) {
            return o.company.trim().to_string();
        }
        if let Some(e) = self.emails.iter().find(|e| !e.address.trim().is_empty()) {
            return e.address.trim().to_string();
        }
        self.phones.iter().find(|p| !p.value.trim().is_empty()).map(|p| p.value.trim().to_string()).unwrap_or_default()
    }

    /// What the contact files under ("Smith, Anna"), lower-cased for sorting.
    pub fn sort_key(&self) -> String {
        if !self.name.file_as.trim().is_empty() {
            return self.name.file_as.trim().to_lowercase();
        }
        self.display_name().to_lowercase()
    }

    pub fn company(&self) -> &str {
        self.organizations.first().map(|o| o.company.as_str()).unwrap_or("")
    }

    pub fn job_title(&self) -> &str {
        self.organizations.first().map(|o| o.title.as_str()).unwrap_or("")
    }

    /// Up to two letters for an avatar.
    pub fn initials(&self) -> String {
        if !self.name.initials.trim().is_empty() {
            return self.name.initials.trim().chars().filter(|c| c.is_alphanumeric()).take(2).collect::<String>().to_uppercase();
        }
        let given = self.name.given.trim().chars().next();
        let family = self.name.family.trim().chars().next();
        if self.name.display.trim().is_empty() {
            if let (Some(a), Some(b)) = (given, family) {
                return format!("{}{}", a.to_uppercase(), b.to_uppercase());
            }
        }
        let name = self.display_name();
        let words: Vec<&str> = name.split_whitespace().filter(|w| w.chars().next().is_some_and(|c| c.is_alphabetic())).collect();
        match words.as_slice() {
            [] => String::new(),
            [one] => one.chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_default(),
            [first, .., last] => format!("{}{}", first.chars().next().unwrap_or(' ').to_uppercase(), last.chars().next().unwrap_or(' ').to_uppercase()),
        }
    }

    /// The phone keys of this contact (see [`phone_key`]).
    pub fn phone_keys(&self) -> Vec<String> {
        let mut v: Vec<String> = self.phones.iter().filter_map(|p| phone_key(&p.value)).collect();
        v.dedup();
        v
    }

    pub fn email_keys(&self) -> Vec<String> {
        self.emails.iter().map(|e| e.address.trim().to_lowercase()).filter(|e| !e.is_empty()).collect()
    }

    /// Every word a search can match, lower-cased.
    pub fn search_text(&self) -> String {
        let mut s = String::new();
        let mut push = |t: &str| {
            if !t.trim().is_empty() {
                s.push_str(&t.to_lowercase());
                s.push('\n');
            }
        };
        push(&self.display_name());
        let n = &self.name;
        for t in [&n.given, &n.middle, &n.family, &n.nickname, &n.maiden, &n.phonetic_given, &n.phonetic_family, &n.file_as] {
            push(t);
        }
        for o in &self.organizations {
            push(&o.company);
            push(&o.department);
            push(&o.title);
        }
        push(&self.occupation);
        for e in &self.emails {
            push(&e.address);
        }
        for p in &self.phones {
            push(&p.value);
            push(&p.value.chars().filter(|c| c.is_ascii_digit()).collect::<String>());
        }
        for a in &self.addresses {
            push(&a.lines().join(" "));
        }
        for l in &self.labels {
            push(l);
        }
        for r in &self.relations {
            push(&r.value);
        }
        for c in &self.custom_fields {
            push(&c.value);
        }
        push(&self.notes);
        s
    }

    /// Whether it holds anything worth saving.
    pub fn is_blank(&self) -> bool {
        self.display_name().is_empty() && self.notes.trim().is_empty() && self.photo.is_empty()
    }
}

/// The digits that identify a phone number: the last ten, so "+1 (555) 010-2030" and
/// "555-010-2030" match. None for anything shorter than seven digits.
pub fn phone_key(number: &str) -> Option<String> {
    let digits: String = number.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() < 7 {
        return None;
    }
    Some(digits[digits.len().saturating_sub(10)..].to_string())
}

/// The standard labels the editor offers for each list (the person can type any other).
pub mod labels {
    pub const PHONE: &[&str] = &[
        "mobile", "home", "work", "main", "other", "iPhone", "home fax", "work fax", "other fax", "pager", "work mobile", "work pager", "assistant", "callback", "car", "company main", "ISDN",
        "radio", "telex", "TTY/TDD", "Google Voice", "primary",
    ];
    pub const EMAIL: &[&str] = &["home", "work", "other", "school", "iCloud"];
    pub const ADDRESS: &[&str] = &["home", "work", "other", "school"];
    pub const WEBSITE: &[&str] = &["home page", "work", "blog", "profile", "portfolio", "FTP", "other"];
    pub const SOCIAL: &[&str] = &["LinkedIn", "Facebook", "Instagram", "X (Twitter)", "Mastodon", "Bluesky", "Threads", "TikTok", "YouTube", "GitHub", "Reddit", "other"];
    pub const IM_PROTOCOL: &[&str] = &["Teams", "Skype", "WhatsApp", "Signal", "Telegram", "Discord", "Slack", "Google Chat", "Jabber", "AIM", "Yahoo", "ICQ", "QQ", "WeChat", "Line", "other"];
    pub const RELATION: &[&str] = &[
        "spouse", "partner", "domestic partner", "child", "son", "daughter", "mother", "father", "parent", "brother", "sister", "sibling", "grandparent", "grandchild", "friend", "relative",
        "manager", "assistant", "referred by", "colleague", "emergency contact", "other",
    ];
    pub const EVENT: &[&str] = &["anniversary", "first met", "graduation", "wedding", "memorial", "other"];
    pub const ORGANIZATION: &[&str] = &["work", "school", "volunteer", "other"];
    pub const EXTERNAL_ID: &[&str] = &["account", "customer", "network", "organization", "employee", "government", "other"];
    pub const LOCATION: &[&str] = &["desk", "building", "floor", "office", "other"];
    pub const CALENDAR: &[&str] = &["home", "work", "free/busy", "availability", "other"];
    pub const KEY: &[&str] = &["PGP", "S/MIME", "SSH", "other"];
    pub const SIP: &[&str] = &["work", "home", "mobile", "other"];
    pub const CUSTOM_KIND: &[&str] = &["text", "number", "date", "url", "email", "phone", "yesno"];
    pub const GENDER: &[&str] = &["", "female", "male", "non-binary", "other", "prefer not to say"];
    pub const SENSITIVITY: &[&str] = &["normal", "personal", "private", "confidential"];
    pub const PRIORITY: &[&str] = &["normal", "low", "high"];
    pub const KIND: &[&str] = &["individual", "organization", "group", "location"];
    pub const THEME_STYLE: &[&str] = &["gradient", "solid", "diagonal", "stripes", "dots", "glow"];
    pub const AVATAR_SHAPE: &[&str] = &["circle", "rounded", "square"];
    pub const BACKGROUND_FIT: &[&str] = &["cover", "contain", "tile", "center", "stretch"];
    /// Fonts every Windows 10/11 computer has: the family name and its file in `C:\Windows\Fonts`.
    pub const WINDOWS_FONTS: &[(&str, &str)] = &[
        ("Segoe UI", "segoeui.ttf"),
        ("Arial", "arial.ttf"),
        ("Bahnschrift", "bahnschrift.ttf"),
        ("Calibri", "calibri.ttf"),
        ("Cambria", "cambria.ttc"),
        ("Candara", "Candara.ttf"),
        ("Comic Sans MS", "comic.ttf"),
        ("Consolas", "consola.ttf"),
        ("Constantia", "constan.ttf"),
        ("Corbel", "corbel.ttf"),
        ("Courier New", "cour.ttf"),
        ("Franklin Gothic Medium", "framd.ttf"),
        ("Gabriola", "Gabriola.ttf"),
        ("Georgia", "georgia.ttf"),
        ("Impact", "impact.ttf"),
        ("Ink Free", "Inkfree.ttf"),
        ("Lucida Console", "lucon.ttf"),
        ("Lucida Sans Unicode", "l_10646.ttf"),
        ("Palatino Linotype", "pala.ttf"),
        ("Segoe Print", "segoepr.ttf"),
        ("Segoe Script", "segoesc.ttf"),
        ("Sylfaen", "sylfaen.ttf"),
        ("Tahoma", "tahoma.ttf"),
        ("Times New Roman", "times.ttf"),
        ("Trebuchet MS", "trebuc.ttf"),
        ("Verdana", "verdana.ttf"),
    ];
}
