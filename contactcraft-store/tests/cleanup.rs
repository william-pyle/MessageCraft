//! The clean-up rules and the duplicate finder.

use std::collections::{HashMap, HashSet};

use contactcraft_store::cleanup::{self, NamePart, Options};
use contactcraft_store::{Address, Contact, Email, Labeled};

#[test]
fn names() {
    assert_eq!(cleanup::proper_case("JOHN", NamePart::Given), "John");
    assert_eq!(cleanup::proper_case("o'brien", NamePart::Family), "O'Brien");
    assert_eq!(cleanup::proper_case("mcdonald", NamePart::Family), "McDonald");
    assert_eq!(cleanup::proper_case("anne-marie", NamePart::Given), "Anne-Marie");
    assert_eq!(cleanup::proper_case("DeShawn", NamePart::Given), "DeShawn");
    assert_eq!(cleanup::proper_case("van der berg", NamePart::Family), "van der Berg");
    assert_eq!(cleanup::proper_case("iii", NamePart::Suffix), "III");
    assert_eq!(cleanup::proper_case("phd", NamePart::Suffix), "PhD");
    assert_eq!(cleanup::proper_case("DJ", NamePart::Given), "DJ");
    assert_eq!(cleanup::strip_symbols("**Anna** (work) 😀", false), "Anna work");
    assert_eq!(cleanup::strip_symbols("José  O’Neil-", false), "José O'Neil");
    assert_eq!(cleanup::strip_symbols("Bob #2", false), "Bob");
}

#[test]
fn phones() {
    assert_eq!(cleanup::format_phone("5550102030"), "(555) 010-2030");
    assert_eq!(cleanup::format_phone("555.010.2030"), "(555) 010-2030");
    assert_eq!(cleanup::format_phone("+1 555-010-2030"), "+1 (555) 010-2030");
    assert_eq!(cleanup::format_phone("1-555-010-2030 x45"), "+1 (555) 010-2030 ext. 45");
    assert_eq!(cleanup::format_phone("+44 (0)7700-900123"), "+44 7700 900123");
    assert_eq!(cleanup::format_phone("0102030"), "010-2030");
    assert_eq!(cleanup::format_phone("911"), "911");
    assert_eq!(cleanup::format_phone("1-800-FLOWERS"), "1-800-FLOWERS");
}

#[test]
fn addresses() {
    let a = Address { street: "123  MAIN STREET".into(), extended: "apartment 4b".into(), city: "SPRINGFIELD".into(), region: "illinois".into(), postal_code: "627011234".into(), country: "usa".into(), ..Default::default() };
    let b = cleanup::format_address(&a);
    assert_eq!(b.street, "123 Main St");
    assert_eq!(b.extended, "Apt 4b");
    assert_eq!(b.city, "Springfield");
    assert_eq!(b.region, "IL");
    assert_eq!(b.postal_code, "62701-1234");
    assert_eq!(b.country, "United States");
    assert_eq!(b.country_code, "US");
    let c = cleanup::format_address(&Address { postal_code: "k1a0b1".into(), country: "Canada".into(), region: "on".into(), ..Default::default() });
    assert_eq!(c.postal_code, "K1A 0B1");
    assert_eq!(c.region, "ON");
}

#[test]
fn repeats_on_one_contact() {
    let mut c = Contact::new();
    c.name.given = "ANNA".into();
    c.emails = vec![
        Email { label: "home".into(), address: "anna@example.com".into(), display_name: String::new() },
        Email { label: "work".into(), address: "Anna@Example.com ".into(), display_name: "Anna S".into() },
        Email::default(),
    ];
    c.phones = vec![Labeled::new("mobile", "555-010-2030"), Labeled::new("other", "+1 (555) 010 2030"), Labeled::new("work", "555-010-2030 ext 9")];
    let ch = cleanup::clean(&mut c, &Options::default());
    assert!(ch.names);
    assert_eq!(c.name.given, "Anna");
    assert_eq!(c.emails.len(), 1);
    assert_eq!(c.emails[0].display_name, "Anna S");
    assert_eq!(c.phones.len(), 2, "{:?}", c.phones);
    assert_eq!(c.phones[0].value, "(555) 010-2030");
}

fn person(given: &str, family: &str, phone: &str) -> Contact {
    let mut c = Contact::new();
    c.id = contactcraft_store::new_id();
    c.name.given = given.into();
    c.name.family = family.into();
    if !phone.is_empty() {
        c.phones.push(Labeled::new("mobile", phone));
    }
    c
}

#[test]
fn duplicates_and_merge() {
    let a = person("Anna", "Smith", "555-010-2030");
    let mut b = person("anna", "smith", "");
    b.birthday = "1990-01-02".into();
    b.emails.push(Email { label: "home".into(), address: "anna@example.com".into(), display_name: String::new() });
    let c = person("Sam", "Okafor", "(555) 010-2030");
    let d = person("Lee", "Park", "555-777-8888");
    let list = vec![a.clone(), b.clone(), c.clone(), d.clone()];
    let groups = cleanup::duplicate_groups(&list, &HashSet::new());
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].members, vec![0, 1, 2]);
    assert!(groups[0].same_name && groups[0].same_number);

    // Told they're different people: Sam no longer joins Anna's group.
    let mut apart = HashSet::new();
    apart.insert((c.id.clone(), a.id.clone()));
    let groups = cleanup::duplicate_groups(&list, &apart);
    assert_eq!(groups[0].members, vec![0, 1]);

    // A real disagreement (two birthdays) is a conflict; capitals alone, or one side empty, aren't.
    let mut a2 = a.clone();
    a2.birthday = "1990-01-03".into();
    let pair = vec![a2.clone(), b.clone()];
    assert!(!cleanup::identical(&pair));
    let conf = cleanup::conflicts(&pair);
    let paths: Vec<&String> = conf.iter().map(|x| &x.path).collect();
    assert_eq!(paths, vec!["birthday"]);
    let mut picks = HashMap::new();
    picks.insert("birthday".to_string(), 1);
    let m = cleanup::merge(&pair, 0, &picks).contact;
    assert_eq!(m.id, a2.id);
    assert_eq!(m.name.given, "Anna");
    assert_eq!(m.birthday, "1990-01-02");
    assert_eq!(m.phones.len(), 1);
    assert_eq!(m.emails.len(), 1, "the email from the other entry is carried over");
    // Exactly the same apart from ids and times: identical.
    let mut e = a.clone();
    e.id = contactcraft_store::new_id();
    e.created = "2020-01-01T00:00:00Z".into();
    assert!(cleanup::identical(&[a.clone(), e]));
}
