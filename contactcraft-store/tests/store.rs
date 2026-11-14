//! The store, the importers and the exporters, checked end to end in a temporary folder.

use contactcraft_store::{Contact, ContactTheme, Email, Labeled, Store, csv, vcard};

fn temp_store(name: &str) -> (Store, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("contactcraft-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    (Store::open(&dir).unwrap(), dir)
}

#[test]
fn save_find_change_delete() {
    let (store, dir) = temp_store("basic");
    let mut c = Contact::new();
    c.name.given = "Anna".into();
    c.name.family = "Smith".into();
    c.phones.push(Labeled::new("mobile", "+1 (555) 010-2030"));
    c.emails.push(Email { label: "home".into(), address: "Anna@Example.com".into(), display_name: String::new() });
    c.theme = ContactTheme { accent: "#112233".into(), font: "Georgia".into(), font_file: "georgia.ttf".into(), text_size: 16.0, ..Default::default() };
    c.extra.insert("messagecraftNote".into(), serde_json::json!("kept"));
    let r0 = store.revision();
    store.save(&mut c).unwrap();
    assert!(!c.id.is_empty());
    assert!(store.revision() > r0);

    // Another program opening the same folder sees it, by phone or email in any form.
    let other = Store::open(&dir).unwrap();
    let found = other.find_by_phone("555-010-2030").unwrap().expect("found by phone");
    assert_eq!(found.display_name(), "Anna Smith");
    assert_eq!(found.theme.text_size, 16.0);
    assert_eq!(found.extra.get("messagecraftNote"), Some(&serde_json::json!("kept")));
    assert!(other.find_by_email("anna@example.com").unwrap().is_some());
    assert_eq!(other.search("smith").unwrap().len(), 1);

    // A change made by the other program shows here.
    let mut f = found;
    f.favorite = true;
    other.save(&mut f).unwrap();
    assert!(store.get(&c.id).unwrap().unwrap().favorite);
    assert_eq!(store.changes_since(r0).unwrap().len(), 1);

    store.delete(&c.id).unwrap();
    assert!(store.list().unwrap().is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn google_csv() {
    let text = "First Name,Middle Name,Last Name,Nickname,Organization Name,Organization Title,Birthday,Notes,Labels,E-mail 1 - Label,E-mail 1 - Value,Phone 1 - Label,Phone 1 - Value,Phone 2 - Label,Phone 2 - Value,Address 1 - Label,Address 1 - Street,Address 1 - City,Address 1 - Region,Address 1 - Postal Code,Address 1 - Country,Relation 1 - Label,Relation 1 - Value,Event 1 - Label,Event 1 - Value,Custom Field 1 - Label,Custom Field 1 - Value,Website 1 - Label,Website 1 - Value,Something New\r\n\
Avery,,Lindqvist,Ave,Northwind,Art Director,1988-04-12,\"Line one\nline two, with comma\",* myContacts ::: Friends ::: * starred,* Home,avery@example.com ::: ave@example.org,Mobile,+1 555 010 2030,Work,+1 555 010 4400,Home,12 Harbor Lane,Portland,OR,97201,US,Partner,Sam Okafor,Anniversary,2015-06-20,Shirt size,M,Blog,https://ave.example,surprise\r\n";
    let items = csv::import(text);
    assert_eq!(items.len(), 1);
    let c = &items[0].0;
    assert_eq!(c.name.given, "Avery");
    assert_eq!(c.name.nickname, "Ave");
    assert_eq!(c.emails.len(), 2);
    assert_eq!(c.emails[0].label, "home");
    assert_eq!(c.phones.len(), 2);
    assert_eq!(c.addresses[0].city, "Portland");
    assert_eq!(c.relations[0].value, "Sam Okafor");
    assert_eq!(c.anniversary, "2015-06-20");
    assert!(c.favorite);
    assert_eq!(c.labels, vec!["Friends".to_string()]);
    assert_eq!(c.notes, "Line one\nline two, with comma");
    assert!(c.custom_fields.iter().any(|f| f.label == "Shirt size" && f.value == "M"));
    assert!(c.custom_fields.iter().any(|f| f.label == "Something New" && f.value == "surprise"));
    assert_eq!(c.company(), "Northwind");

    // And out again: Google CSV it can read back.
    let back = csv::import(&csv::export_google(&[c.clone()]));
    assert_eq!(back[0].0.emails.len(), 2);
    assert_eq!(back[0].0.addresses[0].postal_code, "97201");
}

#[test]
fn outlook_csv() {
    let text = "Title,First Name,Middle Name,Last Name,Suffix,Company,Department,Job Title,Business Street,Business City,Business State,Business Postal Code,Business Country/Region,Home Phone,Mobile Phone,Business Fax,E-mail Address,E-mail Display Name,E-mail 2 Address,Birthday,Spouse,Children,Categories,Billing Information,User 1,Sensitivity,Notes\r\n\
Dr.,Rafael,,Moreno,,Riverside Clinic,Medicine,Physician,1 Main St,Springfield,IL,62701,United States,555-0101,555-0102,555-0103,r@clinic.example,Dr. Moreno,rafa@home.example,4/12/1975,Ana,\"Leo, Mia\",Health;Doctors,Insurance on file,Blue,Normal,Hi\r\n";
    let items = csv::import(text);
    assert_eq!(items.len(), 1);
    let c = &items[0].0;
    assert_eq!(c.name.prefix, "Dr.");
    assert_eq!(c.organizations[0].title, "Physician");
    assert_eq!(c.addresses[0].label, "work");
    assert_eq!(c.addresses[0].region, "IL");
    assert!(c.phones.iter().any(|p| p.label == "work fax"));
    assert_eq!(c.emails[0].display_name, "Dr. Moreno");
    assert_eq!(c.emails.len(), 2);
    assert_eq!(c.birthday, "1975-04-12");
    assert_eq!(c.relations.iter().filter(|r| r.label == "child").count(), 2);
    assert_eq!(c.labels, vec!["Health".to_string(), "Doctors".to_string()]);
    assert_eq!(c.outlook.billing_information, "Insurance on file");
    assert_eq!(c.outlook.user1, "Blue");
}

#[test]
fn vcard_round_trip() {
    let mut c = Contact::new();
    c.name.given = "Mira".into();
    c.name.family = "Chen".into();
    c.phones.push(Labeled::new("mobile", "+1 555 018 2211"));
    c.addresses.push(contactcraft_store::Address { label: "home".into(), street: "5 Elm; Apt 2".into(), city: "Austin".into(), ..Default::default() });
    c.birthday = "--09-30".into();
    c.theme.accent = "#8E2C7A".into();
    c.theme.background_opacity = 40;
    c.outlook.mileage = "120".into();
    let text = vcard::export(&[c.clone()], |_| Some(vec![0x89, b'P', b'N', b'G', 1, 2, 3]));
    let back = vcard::import(&text);
    assert_eq!(back.len(), 1);
    let (b, photo) = &back[0];
    assert_eq!(b.display_name(), "Mira Chen");
    assert_eq!(b.addresses[0].street, "5 Elm; Apt 2");
    assert_eq!(b.theme.accent, "#8E2C7A");
    assert_eq!(b.theme.background_opacity, 40);
    assert_eq!(b.outlook.mileage, "120");
    assert!(photo.is_some());

    // A plain vCard from another program (Apple's labels, folded lines).
    let apple = "BEGIN:VCARD\r\nVERSION:3.0\r\nN:Okafor;Sam;;;\r\nFN:Sam Okafor\r\nTEL;type=CELL;type=VOICE;type=pref:+1 555 010 9999\r\nitem1.X-ABDATE:2015-06-20\r\nitem1.X-ABLABEL:_$!<Anniversary>!$_\r\nNOTE:Long note that is\r\n  folded onto two lines\r\nX-CUSTOM-THING:hello\r\nEND:VCARD\r\n";
    let a = vcard::import(apple);
    assert_eq!(a[0].0.phones[0].label, "mobile");
    assert_eq!(a[0].0.events[0].label, "anniversary");
    assert_eq!(a[0].0.notes, "Long note that is folded onto two lines");
    assert!(a[0].0.custom_fields.iter().any(|f| f.value == "hello"));
}
