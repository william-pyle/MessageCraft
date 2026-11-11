//! MessageCraft's icons, drawn in code on a 20×20 grid in one stroke, in the manner of MailCraft's
//! icon set (ink plus one accent part). Original drawings.

use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, pos2, vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Icon {
    Chat,
    NewChat,
    Sync,
    Search,
    Attach,
    Send,
    Trash,
    MarkRead,
    Archive,
    Spam,
    Inbox,
    Gear,
    Phone,
    Person,
    People,
    Reply,
    Copy,
    FolderPane,
    Theme,
    Zoom,
    Spacing,
    Undo,
    Redo,
    Image,
    Close,
    ChevronDown,
    ChevronRight,
    ChevronLeft,
    Check,
    DoubleCheck,
    Warning,
    Unpair,
    Help,
    Info,
    Star,
    Pin,
}

#[allow(dead_code)]
struct Pen<'a> {
    p: &'a Painter,
    o: Pos2,
    k: f32,
    w: f32,
    c: Color32,
    a: Color32,
}

impl Pen<'_> {
    fn pt(&self, x: f32, y: f32) -> Pos2 {
        pos2(self.o.x + x * self.k, self.o.y + y * self.k)
    }
    fn stroke(&self, c: Color32) -> Stroke {
        Stroke::new(self.w, c)
    }
    fn line(&self, pts: &[(f32, f32)]) {
        self.line_c(pts, self.c);
    }
    fn line_c(&self, pts: &[(f32, f32)], c: Color32) {
        let v: Vec<Pos2> = pts.iter().map(|&(x, y)| self.pt(x, y)).collect();
        self.p.add(Shape::line(v, self.stroke(c)));
    }
    fn closed(&self, pts: &[(f32, f32)], c: Color32) {
        let v: Vec<Pos2> = pts.iter().map(|&(x, y)| self.pt(x, y)).collect();
        self.p.add(Shape::closed_line(v, self.stroke(c)));
    }
    fn rect(&self, x0: f32, y0: f32, x1: f32, y1: f32, r: f32, c: Color32) {
        let rect = Rect::from_min_max(self.pt(x0, y0), self.pt(x1, y1));
        self.p.rect_stroke(rect, r * self.k, self.stroke(c), egui::StrokeKind::Middle);
    }
    fn frect(&self, x0: f32, y0: f32, x1: f32, y1: f32, r: f32, c: Color32) {
        let rect = Rect::from_min_max(self.pt(x0, y0), self.pt(x1, y1));
        self.p.rect_filled(rect, r * self.k, c);
    }
    fn circle(&self, x: f32, y: f32, r: f32, c: Color32) {
        self.p.circle_stroke(self.pt(x, y), r * self.k, self.stroke(c));
    }
    fn dot(&self, x: f32, y: f32, r: f32, c: Color32) {
        self.p.circle_filled(self.pt(x, y), r * self.k, c);
    }
    fn arc(&self, cx: f32, cy: f32, r: f32, a0: f32, a1: f32, c: Color32) {
        let n = 24;
        let pts: Vec<Pos2> = (0..=n)
            .map(|i| {
                let t = a0 + (a1 - a0) * i as f32 / n as f32;
                self.pt(cx + r * t.cos(), cy + r * t.sin())
            })
            .collect();
        self.p.add(Shape::line(pts, self.stroke(c)));
    }
    /// A speech bubble: a rounded box with a tail at the bottom left.
    fn bubble(&self, x0: f32, y0: f32, x1: f32, y1: f32, c: Color32) {
        self.rect(x0, y0, x1, y1, 2.5, c);
        self.p.add(Shape::convex_polygon(
            vec![self.pt(x0 + 2.0, y1 - 0.5), self.pt(x0 + 5.5, y1 - 0.5), self.pt(x0 + 1.5, y1 + 3.0)],
            c,
            Stroke::NONE,
        ));
    }
}

/// Draw `icon` fitted into `rect` in ink `c` with accent `a`.
pub fn paint(p: &Painter, rect: Rect, icon: Icon, c: Color32, a: Color32) {
    let size = rect.width().min(rect.height());
    if size <= 0.0 {
        return;
    }
    let k = size / 20.0;
    let o = rect.center() - vec2(size, size) / 2.0;
    let pen = Pen { p, o, k, w: (1.25 * k).max(1.0), c, a };
    let s = &pen;
    use std::f32::consts::{FRAC_PI_2, PI};
    match icon {
        Icon::Chat => {
            s.bubble(2.5, 3.5, 17.5, 14.0, c);
            s.dot(7.0, 8.75, 0.95, a);
            s.dot(10.0, 8.75, 0.95, a);
            s.dot(13.0, 8.75, 0.95, a);
        }
        Icon::NewChat => {
            s.bubble(2.5, 4.5, 14.5, 14.5, c);
            s.line_c(&[(16.0, 1.5), (16.0, 7.5)], a);
            s.line_c(&[(13.0, 4.5), (19.0, 4.5)], a);
        }
        Icon::Sync => {
            s.arc(10.0, 10.0, 6.5, -PI * 0.95, -PI * 0.1, c);
            s.arc(10.0, 10.0, 6.5, PI * 0.05, PI * 0.9, c);
            s.line_c(&[(13.4, 2.6), (16.6, 4.2), (15.4, 7.6)], a);
            s.line_c(&[(6.6, 17.4), (3.4, 15.8), (4.6, 12.4)], a);
        }
        Icon::Search => {
            s.circle(8.5, 8.5, 5.5, c);
            s.line_c(&[(12.6, 12.6), (17.5, 17.5)], a);
        }
        Icon::Attach => {
            s.line(&[(13.5, 6.0), (7.5, 12.0)]);
            s.arc(9.25, 13.75, 2.5, PI * 0.75, PI * 1.75, c);
            s.arc(13.0, 5.0, 3.6, -PI * 1.25, -PI * 0.25 + 0.2, c);
            s.line(&[(15.6, 7.6), (8.4, 15.6)]);
            s.line_c(&[(5.5, 11.5), (11.5, 5.5)], a);
        }
        Icon::Send => {
            s.closed(&[(2.5, 3.0), (18.0, 10.0), (2.5, 17.0), (5.0, 10.0)], c);
            s.line_c(&[(5.0, 10.0), (11.0, 10.0)], a);
        }
        Icon::Trash => {
            s.line(&[(3.5, 5.0), (16.5, 5.0)]);
            s.line(&[(7.5, 5.0), (8.0, 2.75), (12.0, 2.75), (12.5, 5.0)]);
            s.closed(&[(5.0, 5.0), (6.0, 17.5), (14.0, 17.5), (15.0, 5.0)], c);
            s.line_c(&[(8.5, 8.5), (8.5, 14.0)], a);
            s.line_c(&[(11.5, 8.5), (11.5, 14.0)], a);
        }
        Icon::MarkRead => {
            s.bubble(2.5, 3.5, 17.5, 14.0, c);
            s.line_c(&[(6.5, 8.75), (9.0, 11.0), (13.5, 6.5)], a);
        }
        Icon::Archive => {
            s.rect(2.5, 3.5, 17.5, 7.0, 1.0, c);
            s.line(&[(4.0, 7.0), (4.0, 16.5), (16.0, 16.5), (16.0, 7.0)]);
            s.line_c(&[(8.0, 10.0), (12.0, 10.0)], a);
        }
        Icon::Spam => {
            s.closed(&[(10.0, 2.0), (16.5, 4.5), (16.0, 11.0), (10.0, 18.0), (4.0, 11.0), (3.5, 4.5)], c);
            s.line_c(&[(10.0, 6.0), (10.0, 10.5)], a);
            s.dot(10.0, 13.25, 1.0, a);
        }
        Icon::Inbox => {
            s.closed(&[(2.5, 10.5), (5.0, 3.5), (15.0, 3.5), (17.5, 10.5), (17.5, 16.5), (2.5, 16.5)], c);
            s.line_c(&[(2.5, 10.5), (7.0, 10.5), (8.0, 12.5), (12.0, 12.5), (13.0, 10.5), (17.5, 10.5)], a);
        }
        Icon::Gear => {
            for i in 0..8 {
                let t = i as f32 * PI / 4.0;
                s.line(&[(10.0 + 5.5 * t.cos(), 10.0 + 5.5 * t.sin()), (10.0 + 7.75 * t.cos(), 10.0 + 7.75 * t.sin())]);
            }
            s.circle(10.0, 10.0, 5.25, c);
            s.circle(10.0, 10.0, 2.0, a);
        }
        Icon::Phone => {
            s.rect(5.5, 1.75, 14.5, 18.25, 2.0, c);
            s.line_c(&[(8.75, 15.5), (11.25, 15.5)], a);
        }
        Icon::Person => {
            s.circle(10.0, 6.5, 3.5, c);
            s.arc(10.0, 18.5, 6.5, -PI, 0.0, c);
        }
        Icon::People => {
            s.circle(7.5, 7.0, 3.0, c);
            s.arc(7.5, 18.0, 5.5, -PI, 0.0, c);
            s.circle(14.0, 6.0, 2.5, a);
            s.arc(14.5, 15.5, 4.5, -PI * 0.85, 0.0, a);
        }
        Icon::Reply => {
            s.line_c(&[(7.5, 4.5), (2.5, 9.0), (7.5, 13.5)], a);
            s.line(&[(2.5, 9.0), (12.0, 9.0)]);
            s.arc(12.0, 13.0, 4.0, -FRAC_PI_2, FRAC_PI_2 * 0.9, c);
        }
        Icon::Copy => {
            s.rect(6.5, 6.5, 16.5, 17.5, 1.5, c);
            s.line_c(&[(3.5, 13.5), (3.5, 3.5), (12.5, 3.5)], a);
        }
        Icon::FolderPane => {
            s.rect(2.5, 3.5, 17.5, 16.5, 1.5, c);
            s.frect(3.25, 4.25, 7.5, 15.75, 0.5, a.linear_multiply(0.55));
            s.line(&[(7.5, 3.5), (7.5, 16.5)]);
        }
        Icon::Theme => {
            s.circle(10.0, 10.0, 7.0, c);
            let mut pts = vec![s.pt(10.0, 3.0)];
            for i in 0..=16 {
                let t = -FRAC_PI_2 + PI * i as f32 / 16.0;
                pts.push(s.pt(10.0 + 7.0 * t.cos(), 10.0 + 7.0 * t.sin()));
            }
            p.add(Shape::convex_polygon(pts, a, Stroke::NONE));
        }
        Icon::Zoom => {
            s.circle(8.5, 8.5, 5.5, c);
            s.line(&[(12.6, 12.6), (17.5, 17.5)]);
            s.line_c(&[(6.0, 8.5), (11.0, 8.5)], a);
            s.line_c(&[(8.5, 6.0), (8.5, 11.0)], a);
        }
        Icon::Spacing => {
            s.line(&[(7.0, 4.5), (17.0, 4.5)]);
            s.line(&[(7.0, 10.0), (17.0, 10.0)]);
            s.line(&[(7.0, 15.5), (17.0, 15.5)]);
            s.line_c(&[(3.5, 3.0), (3.5, 17.0)], a);
            s.line_c(&[(2.0, 5.0), (3.5, 3.0), (5.0, 5.0)], a);
            s.line_c(&[(2.0, 15.0), (3.5, 17.0), (5.0, 15.0)], a);
        }
        Icon::Undo => {
            s.line_c(&[(6.5, 3.5), (3.0, 7.0), (6.5, 10.5)], a);
            s.arc(11.0, 11.5, 4.5, -FRAC_PI_2, FRAC_PI_2, c);
            s.line(&[(3.0, 7.0), (11.0, 7.0)]);
            s.line(&[(11.0, 16.0), (7.0, 16.0)]);
        }
        Icon::Redo => {
            s.line_c(&[(13.5, 3.5), (17.0, 7.0), (13.5, 10.5)], a);
            s.arc(9.0, 11.5, 4.5, FRAC_PI_2, PI * 1.5, c);
            s.line(&[(17.0, 7.0), (9.0, 7.0)]);
            s.line(&[(9.0, 16.0), (13.0, 16.0)]);
        }
        Icon::Image => {
            s.rect(2.5, 3.5, 17.5, 16.5, 1.5, c);
            s.line_c(&[(2.5, 14.0), (7.0, 9.5), (11.0, 13.5), (13.5, 11.0), (17.5, 15.0)], a);
            s.dot(13.5, 7.25, 1.25, a);
        }
        Icon::Close => {
            s.line(&[(5.0, 5.0), (15.0, 15.0)]);
            s.line(&[(15.0, 5.0), (5.0, 15.0)]);
        }
        Icon::ChevronDown => s.line(&[(5.5, 8.0), (10.0, 12.5), (14.5, 8.0)]),
        Icon::ChevronRight => s.line(&[(8.0, 5.5), (12.5, 10.0), (8.0, 14.5)]),
        Icon::ChevronLeft => s.line(&[(12.0, 5.5), (7.5, 10.0), (12.0, 14.5)]),
        Icon::Check => s.line_c(&[(4.0, 10.5), (8.0, 14.5), (16.0, 6.0)], a),
        Icon::DoubleCheck => {
            s.line_c(&[(1.5, 10.5), (5.0, 14.0), (12.0, 6.5)], a);
            s.line_c(&[(8.5, 13.5), (9.0, 14.0), (17.5, 6.5)], a);
        }
        Icon::Warning => {
            s.closed(&[(10.0, 2.5), (18.0, 17.0), (2.0, 17.0)], c);
            s.line_c(&[(10.0, 7.5), (10.0, 11.5)], a);
            s.dot(10.0, 14.25, 1.0, a);
        }
        Icon::Unpair => {
            s.rect(5.5, 1.75, 14.5, 18.25, 2.0, c);
            s.line_c(&[(7.5, 7.5), (12.5, 12.5)], a);
            s.line_c(&[(12.5, 7.5), (7.5, 12.5)], a);
        }
        Icon::Help => {
            s.circle(10.0, 10.0, 7.5, c);
            s.arc(10.0, 8.0, 2.5, -PI, PI * 0.35, a);
            s.line_c(&[(11.6, 9.9), (10.0, 11.0), (10.0, 12.0)], a);
            s.dot(10.0, 14.5, 0.95, a);
        }
        Icon::Info => {
            s.circle(10.0, 10.0, 7.5, c);
            s.line_c(&[(10.0, 9.0), (10.0, 14.0)], a);
            s.dot(10.0, 6.25, 1.0, a);
        }
        Icon::Star => {
            let mut v = Vec::new();
            for i in 0..10 {
                let r = if i % 2 == 0 { 7.5 } else { 3.2 };
                let t = -FRAC_PI_2 + i as f32 * PI / 5.0;
                v.push((10.0 + r * t.cos(), 10.5 + r * t.sin()));
            }
            s.closed(&v, a);
        }
        Icon::Pin => {
            s.closed(&[(7.0, 2.5), (13.0, 2.5), (12.0, 8.0), (15.0, 11.0), (5.0, 11.0), (8.0, 8.0)], c);
            s.line_c(&[(10.0, 11.0), (10.0, 17.5)], a);
        }
    }
}
