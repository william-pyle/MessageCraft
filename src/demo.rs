//! Sample conversations for trying the window without a phone (`MessageCraft.exe --demo`).
//! Made-up people and numbers.

use crate::app::{App, Conn, Thread};
use crate::model::{Conversation, Message, Participant, Reaction};
use crate::timefmt;

fn person(id: &str, name: &str, number: &str) -> Participant {
    Participant { id: id.into(), name: name.into(), first: name.split(' ').next().unwrap_or("").into(), number: number.into(), ..Default::default() }
}

fn me() -> Participant {
    Participant { id: "me".into(), name: "Me".into(), is_me: true, ..Default::default() }
}

pub fn fill(app: &mut App) {
    let now = timefmt::now_ms();
    let min = 60_000i64;
    let hour = 60 * min;
    let day = 24 * hour;
    let convs: Vec<(Conversation, Vec<(i64, bool, &str, &str)>)> = vec![
        (
            Conversation {
                id: "c1".into(),
                participants: vec![me(), person("p1", "Avery Lin", "(555) 010-2231")],
                kind: "rcs".into(),
                unread: true,
                pinned: true,
                ..Default::default()
            },
            vec![
                (now - 50 * min, true, "me", "Are we still on for lunch tomorrow?"),
                (now - 46 * min, false, "p1", "Yes! 12:30 at the usual place?"),
                (now - 45 * min, true, "me", "Perfect. I'll grab a table."),
                (now - 3 * min, false, "p1", "Running 10 minutes late tomorrow, save me a seat 🙂"),
            ],
        ),
        (
            Conversation { id: "c2".into(), participants: vec![me(), person("p2", "Jordan Price", "(555) 010-7781")], kind: "rcs".into(), ..Default::default() },
            vec![
                (now - 3 * hour, false, "p2", "Did the package arrive?"),
                (now - 2 * hour - 40 * min, true, "me", "It did, thank you! Everything fits."),
                (now - 2 * hour, false, "p2", "Great, enjoy it"),
            ],
        ),
        (
            Conversation {
                id: "c3".into(),
                name: "Weekend hike".into(),
                group: true,
                participants: vec![me(), person("p3", "Sam Ortiz", "(555) 010-4410"), person("p4", "Riley Chen", "(555) 010-9902"), person("p5", "Morgan Blake", "(555) 010-3317")],
                kind: "rcs".into(),
                unread: true,
                ..Default::default()
            },
            vec![
                (now - day - 5 * hour, false, "p3", "Trail opens at 8. Who's driving?"),
                (now - day - 4 * hour, false, "p4", "I can take four people."),
                (now - day - 4 * hour + 2 * min, true, "me", "Count me in, I'll bring snacks."),
                (now - 20 * min, false, "p5", "Forecast says sunny all morning ☀️"),
            ],
        ),
        (
            Conversation { id: "c4".into(), participants: vec![me(), person("p6", "", "72345")], kind: "sms".into(), ..Default::default() },
            vec![(now - 2 * day, false, "p6", "Your verification code is 482913. It expires in 10 minutes.")],
        ),
        (
            Conversation { id: "c5".into(), participants: vec![me(), person("p7", "Taylor Brooks", "(555) 010-5520")], kind: "sms".into(), ..Default::default() },
            vec![
                (now - 4 * day, false, "p7", "Happy birthday!! 🎉"),
                (now - 4 * day + 30 * min, true, "me", "Thank you so much!"),
            ],
        ),
        (
            Conversation { id: "c6".into(), participants: vec![me(), person("p8", "Dr. Patel's Office", "(555) 010-8800")], kind: "sms".into(), ..Default::default() },
            vec![(now - 9 * day, false, "p8", "Reminder: your appointment is on Friday at 9:00 AM. Reply C to confirm.")],
        ),
        (
            Conversation { id: "c7".into(), participants: vec![me(), person("p9", "Casey Morgan", "(555) 010-6643")], kind: "rcs".into(), ..Default::default() },
            vec![(now - 40 * day, true, "me", "Thanks again for helping with the move!")],
        ),
    ];
    for (mut c, msgs) in convs {
        c.folder = "inbox".into();
        let mut thread = Thread { loaded: true, ..Default::default() };
        for (i, (ts, mine, pid, text)) in msgs.iter().enumerate() {
            thread.messages.push(Message {
                id: format!("{}-{i}", c.id),
                conversation_id: c.id.clone(),
                participant_id: if *mine { "me".into() } else { pid.to_string() },
                ts: *ts,
                from_me: *mine,
                text: text.to_string(),
                status: if *mine { "read".into() } else { "received".into() },
                reactions: if c.id == "c3" && i == 2 { Some(vec![Reaction { emoji: "👍".into(), count: 2 }]) } else { None },
                ..Default::default()
            });
        }
        if let Some(last) = thread.messages.last() {
            c.ts = last.ts;
            c.snippet = last.text.clone();
            c.snippet_me = last.from_me;
            c.latest_msg_id = last.id.clone();
            if c.group && !last.from_me {
                c.snippet_from = c.participant(&last.participant_id).map(|p| p.first.clone()).unwrap_or_default();
            }
        }
        app.threads.insert(c.id.clone(), thread);
        app.convs.insert(c.id.clone(), c);
    }
    app.phone = Some("Pixel (demo)".into());
    app.conn = Conn::Connected;
    app.last_sync = Some(now);
    app.typing.insert("c2".into(), std::time::Instant::now() + std::time::Duration::from_secs(3600));
    app.selected = Some("c1".into());
    if let Some(t) = app.threads.get_mut("c1") {
        t.scroll_to_end = true;
    }
}
