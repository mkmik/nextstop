#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Pt { pub x: i32, pub y: i32 }
pub const fn pt(x: i32, y: i32) -> Pt { Pt { x, y } }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Rect { pub x: i32, pub y: i32, pub w: i32, pub h: i32 }
pub const fn rect(x: i32, y: i32, w: i32, h: i32) -> Rect { Rect { x, y, w, h } }

impl Rect {
    pub fn right(&self) -> i32 { self.x + self.w }
    pub fn bottom(&self) -> i32 { self.y + self.h }
    pub fn contains(&self, p: Pt) -> bool { p.x >= self.x && p.y >= self.y && p.x < self.right() && p.y < self.bottom() }
    pub fn inset(&self, n: i32) -> Rect { rect(self.x + n, self.y + n, self.w - 2 * n, self.h - 2 * n) }
    pub fn at(&self, dx: i32, dy: i32) -> Rect { rect(self.x + dx, self.y + dy, self.w, self.h) }
    pub fn intersect(&self, o: Rect) -> Rect {
        let x0 = self.x.max(o.x);
        let y0 = self.y.max(o.y);
        let x1 = self.right().min(o.right());
        let y1 = self.bottom().min(o.bottom());
        rect(x0, y0, (x1 - x0).max(0), (y1 - y0).max(0))
    }
    pub fn is_empty(&self) -> bool { self.w <= 0 || self.h <= 0 }
    pub fn center(&self) -> Pt { pt(self.x + self.w / 2, self.y + self.h / 2) }
}
