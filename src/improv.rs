//! Improv — after Lotus Improv (NeXT, 1991): a worksheet with no A1/B2 anywhere. Data lives in a
//! cube of named *categories*; formulas are written in plain English over the item names
//! ("Revenue = Units * Price") and apply to every cell of that item; the view is pivoted by
//! dragging the category tiles between the row zone and the column zone of the tile bar.
use crate::app::*;
use crate::chrome::*;
use crate::geom::{pt, rect, Pt, Rect};
use crate::paint::*;
use std::collections::HashMap;

const TILE_H: i32 = 21;   // the tile bar
const TILE_W: i32 = 86;   // a category tile — also one row-header column, so tiles sit over them
const HDR_H: i32 = 17;    // one column-header row (one per column category: nested headers)
const ROW_H: i32 = 16;
const COL_W: i32 = 74;
const FP_H: i32 = 96;     // formula pane
const FROW_H: i32 = 15;

#[derive(Clone)]
pub struct Cat { pub name: String, pub items: Vec<String>, pub col: bool }

/// A cell coordinate: one item index per category, in `cats` order.
type Coord = Vec<u8>;

pub struct Improv {
    pub cats: Vec<Cat>,
    vals: HashMap<Coord, f64>,    // what the user typed
    calc: HashMap<Coord, f64>,    // vals plus everything the formulas produce
    targets: Vec<(usize, usize)>, // the (category, item) each formula writes to
    pub formulas: Vec<String>,
    pub err: String,
    sel: Coord,
    edit: Option<String>,       // in-cell entry
    pub fsel: Option<usize>,    // formula row being edited (== formulas.len() adds one)
    fedit: String,
}

impl Default for Improv {
    fn default() -> Self {
        let cat = |name: &str, items: &[&str], col: bool| Cat { name: name.into(), items: items.iter().map(|s| s.to_string()).collect(), col };
        let mut iv = Improv {
            cats: vec![
                cat("Products", &["Widgets", "Gadgets"], false),
                cat("Items", &["Units", "Price", "Revenue", "Costs", "Profit"], false),
                cat("Quarters", &["Q1", "Q2", "Q3", "Q4"], true),
            ],
            vals: seed(), calc: HashMap::new(), targets: vec![],
            formulas: vec!["Revenue = Units * Price".into(), "Profit = Revenue - Costs".into()],
            err: String::new(), sel: vec![0, 0, 0], edit: None, fsel: None, fedit: String::new(),
        };
        iv.recalc();
        iv
    }
}

/// Units, Price and Costs for the two products over the four quarters (Revenue and Profit are formulas).
fn seed() -> HashMap<Coord, f64> {
    let data = [
        [[120.0, 145.0, 160.0, 210.0], [12.5, 12.5, 13.0, 13.0], [900.0, 1000.0, 1150.0, 1400.0]],
        [[80.0, 95.0, 88.0, 130.0], [24.0, 24.0, 25.0, 25.0], [1200.0, 1350.0, 1300.0, 1800.0]],
    ];
    let mut m = HashMap::new();
    for (p, rows) in data.iter().enumerate() {
        for (r, row) in rows.iter().enumerate() {
            let item = if r == 2 { 3 } else { r }; // Units, Price, …, Costs
            for (q, v) in row.iter().enumerate() { m.insert(vec![p as u8, item as u8, q as u8], *v); }
        }
    }
    m
}

fn fmt(v: f64) -> String { if (v - v.round()).abs() < 1e-9 { format!("{}", v.round() as i64) } else { format!("{v:.2}") } }

// ---- formulas ---------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
enum Tok { Num(f64), Name(String), Op(char) }

struct Formula { target: (usize, usize), toks: Vec<Tok> }

// ponytail: item names are single words, so the lexer can take plain identifier runs. Multi-word
// names ("Gross Margin") would need a longest-match against the category items.
fn lex(s: &str) -> Result<Vec<Tok>, String> {
    let ch: Vec<char> = s.chars().collect();
    let (mut out, mut i) = (vec![], 0);
    while i < ch.len() {
        let c = ch[i];
        if c.is_whitespace() { i += 1; }
        else if c.is_ascii_digit() || c == '.' {
            let st = i;
            while i < ch.len() && (ch[i].is_ascii_digit() || ch[i] == '.') { i += 1; }
            let t: String = ch[st..i].iter().collect();
            out.push(Tok::Num(t.parse().map_err(|_| format!("bad number “{t}”"))?));
        } else if c.is_alphabetic() || c == '_' {
            let st = i;
            while i < ch.len() && (ch[i].is_alphanumeric() || ch[i] == '_') { i += 1; }
            out.push(Tok::Name(ch[st..i].iter().collect()));
        } else if "+-*/()".contains(c) { out.push(Tok::Op(c)); i += 1; }
        else { return Err(format!("unexpected “{c}”")); }
    }
    Ok(out)
}

struct Parse<'a> { t: &'a [Tok], i: usize }
impl Parse<'_> {
    fn op(&self, set: &str) -> Option<char> { match self.t.get(self.i) { Some(&Tok::Op(o)) if set.contains(o) => Some(o), _ => None } }
    fn expr(&mut self, v: &dyn Fn(&str) -> Option<f64>) -> Result<f64, String> {
        let mut a = self.term(v)?;
        while let Some(o) = self.op("+-") { self.i += 1; let b = self.term(v)?; a = if o == '+' { a + b } else { a - b }; }
        Ok(a)
    }
    fn term(&mut self, v: &dyn Fn(&str) -> Option<f64>) -> Result<f64, String> {
        let mut a = self.factor(v)?;
        while let Some(o) = self.op("*/") {
            self.i += 1;
            let b = self.factor(v)?;
            if o == '/' && b == 0.0 { return Err("division by zero".into()); }
            a = if o == '*' { a * b } else { a / b };
        }
        Ok(a)
    }
    fn factor(&mut self, v: &dyn Fn(&str) -> Option<f64>) -> Result<f64, String> {
        if self.op("-").is_some() { self.i += 1; return Ok(-self.factor(v)?); }
        if self.op("+").is_some() { self.i += 1; return self.factor(v); }
        match self.t.get(self.i).cloned() {
            Some(Tok::Num(n)) => { self.i += 1; Ok(n) }
            Some(Tok::Name(n)) => { self.i += 1; v(&n).ok_or(format!("no item named “{n}”")) }
            Some(Tok::Op('(')) => {
                self.i += 1;
                let r = self.expr(v)?;
                if self.op(")").is_none() { return Err("missing “)”".into()); }
                self.i += 1;
                Ok(r)
            }
            _ => Err("expected a value".into()),
        }
    }
}

fn eval(toks: &[Tok], v: &dyn Fn(&str) -> Option<f64>) -> Result<f64, String> {
    let mut p = Parse { t: toks, i: 0 };
    let r = p.expr(v)?;
    if p.i != toks.len() { return Err("extra text after the expression".into()); }
    Ok(r)
}

// ---- model ------------------------------------------------------------------------------------

impl Improv {
    pub fn axis(&self, col: bool) -> Vec<usize> { (0..self.cats.len()).filter(|&i| self.cats[i].col == col).collect() }
    /// Every combination of the items of `axis`, in nesting order (outermost first).
    pub fn tuples(&self, axis: &[usize]) -> Vec<Vec<u8>> {
        let mut out: Vec<Vec<u8>> = vec![vec![]];
        for &c in axis {
            let mut next = Vec::new();
            for t in &out { for i in 0..self.cats[c].items.len() { let mut t = t.clone(); t.push(i as u8); next.push(t); } }
            out = next;
        }
        out
    }
    /// How many leaf lines one header cell at `level` of `axis` spans.
    pub fn span(&self, axis: &[usize], level: usize) -> usize { axis[level + 1..].iter().map(|&c| self.cats[c].items.len()).product() }
    fn find(&self, name: &str) -> Option<(usize, usize)> {
        self.cats.iter().enumerate().find_map(|(c, cat)| cat.items.iter().position(|i| i.eq_ignore_ascii_case(name)).map(|i| (c, i)))
    }
    fn parse(&self, text: &str) -> Result<Formula, String> {
        let (lhs, rhs) = text.split_once('=').ok_or("expected “Item = expression”")?;
        let name = lhs.trim();
        let target = self.find(name).ok_or_else(|| format!("no item named “{name}”"))?;
        let toks = lex(rhs)?;
        if toks.is_empty() { return Err("the expression is empty".into()); }
        Ok(Formula { target, toks })
    }
    /// What `name` means for the cell at `k`: the same cell with that category moved to that item.
    fn reference(&self, name: &str, k: &[u8]) -> Option<f64> {
        let (c, i) = self.find(name)?;
        let mut k = k.to_vec();
        k[c] = i as u8;
        Some(self.calc.get(&k).copied().unwrap_or(0.0))
    }
    pub fn value(&self, k: &[u8]) -> Option<f64> { self.calc.get(k).copied() }
    pub fn computed(&self, k: &[u8]) -> bool { self.targets.iter().any(|&(c, i)| k[c] as usize == i) }

    pub fn recalc(&mut self) {
        self.err.clear();
        self.targets.clear();
        self.calc = self.vals.clone();
        let mut fs = vec![];
        for (n, text) in self.formulas.clone().iter().enumerate() {
            if text.trim().is_empty() { continue; }
            match self.parse(text) {
                Ok(f) => { self.targets.push(f.target); fs.push(f); }
                Err(e) => if self.err.is_empty() { self.err = format!("Formula {}: {e}", n + 1) },
            }
        }
        let all = self.tuples(&(0..self.cats.len()).collect::<Vec<_>>());
        // ponytail: naive fixpoint — one pass per formula settles any dependency order, and the
        // bound means a self-referential formula stops instead of hanging.
        for _ in 0..fs.len() {
            for f in &fs {
                let mut out = vec![];
                for k in &all {
                    if k[f.target.0] as usize != f.target.1 { continue; }
                    let r = eval(&f.toks, &|n| self.reference(n, k));
                    match r { Ok(v) => out.push((k.clone(), v)), Err(e) => if self.err.is_empty() { self.err = e } }
                }
                for (k, v) in out { self.calc.insert(k, v); }
            }
        }
    }

    /// Put `cat` on the row or the column axis at position `idx` within it, keeping the cube intact.
    pub fn move_cat(&mut self, cat: usize, to_col: bool, idx: usize) {
        self.cats[cat].col = to_col;
        let mut order: Vec<usize> = (0..self.cats.len()).filter(|&i| i != cat).collect();
        let slots: Vec<usize> = order.iter().enumerate().filter(|(_, &i)| self.cats[i].col == to_col).map(|(p, _)| p).collect();
        order.insert(slots.get(idx).copied().unwrap_or(order.len()), cat);
        let permute = |k: &[u8]| -> Coord { order.iter().map(|&i| k[i]).collect() };
        self.cats = order.iter().map(|&i| self.cats[i].clone()).collect();
        self.vals = self.vals.iter().map(|(k, v)| (permute(k), *v)).collect();
        self.sel = permute(&self.sel);
        self.recalc();
    }
}

// ---- layout -----------------------------------------------------------------------------------

pub struct Lay {
    pub c: Rect,
    pub rows: Vec<usize>, pub cols: Vec<usize>,
    pub rt: Vec<Vec<u8>>, pub ct: Vec<Vec<u8>>,
    pub head_w: i32, pub head_h: i32,
    pub grid: Rect, pub pane: Rect,
}

impl Lay {
    pub fn divider(&self) -> i32 { self.c.x + self.head_w }
    pub fn tile(&self, col: bool, i: usize) -> Rect {
        let x = if col { self.divider() } else { self.c.x } + i as i32 * TILE_W;
        rect(x + 1, self.c.y + 2, TILE_W - 3, TILE_H - 4)
    }
    pub fn cell(&self, r: usize, c: usize) -> Rect { rect(self.grid.x + c as i32 * COL_W, self.grid.y + r as i32 * ROW_H, COL_W, ROW_H) }
    /// The full coordinate of the cell at row line `r`, column line `c`.
    pub fn coord(&self, n: usize, r: usize, c: usize) -> Coord {
        let mut k = vec![0u8; n];
        for (i, &cat) in self.rows.iter().enumerate() { k[cat] = self.rt[r][i]; }
        for (i, &cat) in self.cols.iter().enumerate() { k[cat] = self.ct[c][i]; }
        k
    }
}

impl App {
    pub fn iv_layout(&self) -> Lay {
        let c = self.content_rect(WinKind::Improv);
        let iv = &self.improv;
        let (rows, cols) = (iv.axis(false), iv.axis(true));
        let (rt, ct) = (iv.tuples(&rows), iv.tuples(&cols));
        // an empty row axis still keeps a corner block, so there is somewhere to drop a tile back
        let head_w = rows.len().max(1) as i32 * TILE_W;
        let head_h = TILE_H + cols.len() as i32 * HDR_H;
        let pane = rect(c.x, c.bottom() - FP_H, c.w, FP_H);
        let grid = rect(c.x + head_w, c.y + head_h, c.w - head_w, pane.y - (c.y + head_h));
        Lay { c, rows, cols, rt, ct, head_w, head_h, grid, pane }
    }
    fn iv_flist(&self) -> Rect { let p = self.iv_layout().pane; rect(p.x + 8, p.y + 20, p.w - 16, p.h - 44) }
    fn iv_frow(&self, i: usize) -> Rect { let l = self.iv_flist(); rect(l.x + 2, l.y + 2 + i as i32 * FROW_H, l.w - 4, FROW_H) }

    // ---- interaction --------------------------------------------------------------------------

    pub fn iv_mouse_down(&mut self, p: Pt) {
        let lay = self.iv_layout();
        for col in [false, true] {
            let axis = self.improv.axis(col);
            if let Some(i) = (0..axis.len()).find(|&i| lay.tile(col, i).contains(p)) {
                let r = lay.tile(col, i);
                self.iv_commit_formula();
                self.capture = Some(Capture::ImprovTile { cat: axis[i], grab: pt(p.x - r.x, p.y - r.y), at: p });
                return;
            }
        }
        if lay.pane.contains(p) {
            let list = self.iv_flist();
            if let Some(i) = (0..=self.improv.formulas.len()).find(|&i| list.contains(p) && self.iv_frow(i).contains(p)) {
                self.iv_commit_formula();
                self.improv.fsel = Some(i);
                self.improv.fedit = self.improv.formulas.get(i).cloned().unwrap_or_default();
            }
            return;
        }
        self.iv_commit_formula();
        if lay.grid.contains(p) {
            let (r, c) = (((p.y - lay.grid.y) / ROW_H) as usize, ((p.x - lay.grid.x) / COL_W) as usize);
            if r < lay.rt.len() && c < lay.ct.len() {
                self.iv_commit_cell();
                self.improv.sel = lay.coord(self.improv.cats.len(), r, c);
            }
        }
    }

    pub fn iv_tile_drop(&mut self, cat: usize, origin: Pt) {
        let lay = self.iv_layout();
        let mid = origin.x + TILE_W / 2;
        let to_col = mid >= lay.divider();
        let zone = if to_col { lay.divider() } else { lay.c.x };
        self.improv.move_cat(cat, to_col, ((mid - zone).max(0) / TILE_W) as usize);
    }

    pub fn iv_key(&mut self, k: Key, _mods: Mods) {
        if self.improv.fsel.is_some() {
            match k {
                Key::Char(c) => self.improv.fedit.push(c),
                Key::Backspace | Key::Delete => { self.improv.fedit.pop(); }
                Key::Enter | Key::Tab => self.iv_commit_formula(),
                Key::Escape => { self.improv.fsel = None; self.improv.fedit.clear(); }
                _ => {}
            }
            return;
        }
        let lay = self.iv_layout();
        let (mut r, mut c) = self.iv_sel_index(&lay);
        let last = (lay.rt.len() - 1, lay.ct.len() - 1);
        match k {
            Key::Char(ch) if ch.is_ascii_digit() || ch == '.' || ch == '-' => {
                let sel = self.improv.sel.clone();
                if self.improv.computed(&sel) {
                    let name = self.iv_sel_name();
                    self.log(format!("“{name}” is computed by a formula"));
                } else { self.improv.edit.get_or_insert_with(String::new).push(ch); }
            }
            Key::Backspace => { if let Some(e) = &mut self.improv.edit { e.pop(); } }
            Key::Escape => self.improv.edit = None,
            Key::Delete => {
                let sel = self.improv.sel.clone();
                self.improv.edit = None;
                if !self.improv.computed(&sel) { self.improv.vals.remove(&sel); self.improv.recalc(); }
            }
            Key::Enter | Key::Down => { self.iv_commit_cell(); r = (r + 1).min(last.0); }
            Key::Up => { self.iv_commit_cell(); r = r.saturating_sub(1); }
            Key::Left => { self.iv_commit_cell(); c = c.saturating_sub(1); }
            Key::Right | Key::Tab => { self.iv_commit_cell(); c = (c + 1).min(last.1); }
            _ => {}
        }
        self.improv.sel = lay.coord(self.improv.cats.len(), r, c);
    }

    fn iv_sel_index(&self, lay: &Lay) -> (usize, usize) {
        let at = |axis: &[usize], ts: &[Vec<u8>]| ts.iter().position(|t| axis.iter().enumerate().all(|(i, &c)| t[i] == self.improv.sel[c])).unwrap_or(0);
        (at(&lay.rows, &lay.rt), at(&lay.cols, &lay.ct))
    }
    fn iv_sel_name(&self) -> String {
        let iv = &self.improv;
        iv.targets.iter().find(|&&(c, i)| iv.sel[c] as usize == i).map_or(String::new(), |&(c, i)| iv.cats[c].items[i].clone())
    }
    fn iv_commit_cell(&mut self) {
        let Some(text) = self.improv.edit.take() else { return };
        let key = self.improv.sel.clone();
        match text.trim().parse::<f64>() {
            Ok(v) => { self.improv.vals.insert(key, v); }
            Err(_) if text.trim().is_empty() => { self.improv.vals.remove(&key); }
            Err(_) => return,
        }
        self.improv.recalc();
    }
    fn iv_commit_formula(&mut self) {
        let Some(i) = self.improv.fsel.take() else { return };
        let text = std::mem::take(&mut self.improv.fedit).trim().to_string();
        if i < self.improv.formulas.len() {
            if text.is_empty() { self.improv.formulas.remove(i); } else { self.improv.formulas[i] = text; }
        } else if !text.is_empty() { self.improv.formulas.push(text); }
        self.improv.recalc();
    }

    // ---- drawing ------------------------------------------------------------------------------

    pub fn iv_draw(&self, p: &mut Painter, c: Rect) {
        let lay = self.iv_layout();
        let iv = &self.improv;
        // tile bar: the row zone left of the divider, the column zone right of it
        p.fill(rect(c.x, c.y, c.w, TILE_H), LIGHT);
        p.vline(lay.divider() - 2, c.y, TILE_H - 2, DARK); p.vline(lay.divider() - 1, c.y, TILE_H - 2, WHITE);
        p.hline(c.x, c.y + TILE_H - 2, c.w, DARK); p.hline(c.x, c.y + TILE_H - 1, c.w, WHITE);
        let dragged = match &self.capture { Some(Capture::ImprovTile { cat, .. }) => Some(*cat), _ => None };
        for col in [false, true] {
            for (i, &cat) in iv.axis(col).iter().enumerate() {
                if dragged == Some(cat) { continue; }
                self.iv_tile(p, lay.tile(col, i), &iv.cats[cat].name);
            }
        }
        // corner block, column headers, row headers — raised cells, as Improv's were
        let corner = rect(c.x, c.y + TILE_H, lay.head_w, lay.head_h - TILE_H);
        p.fill(corner, LIGHT); p.bevel(corner, WHITE, DARK);
        p.push_clip(rect(lay.grid.x, corner.y, c.right() - lay.grid.x, corner.h));
        for (l, &cat) in lay.cols.iter().enumerate() {
            let span = iv.span(&lay.cols, l);
            for (i, t) in lay.ct.iter().enumerate().step_by(span) {
                let r = rect(lay.grid.x + i as i32 * COL_W, corner.y + l as i32 * HDR_H, span as i32 * COL_W, HDR_H);
                p.fill(r, LIGHT); p.bevel(r, WHITE, DARK);
                p.text_in(FontId::Regular, 12, r, Align::Center, &iv.cats[cat].items[t[l] as usize], BLACK);
            }
        }
        p.pop_clip();
        p.push_clip(rect(c.x, lay.grid.y, lay.head_w, lay.grid.h));
        for (l, &cat) in lay.rows.iter().enumerate() {
            let span = iv.span(&lay.rows, l);
            for (i, t) in lay.rt.iter().enumerate().step_by(span) {
                let r = rect(c.x + l as i32 * TILE_W, lay.grid.y + i as i32 * ROW_H, TILE_W, span as i32 * ROW_H);
                p.fill(r, LIGHT); p.bevel(r, WHITE, DARK);
                let name = p.ellipsize(FontId::Regular, 12, &iv.cats[cat].items[t[l] as usize], r.w - 12);
                p.text_in(FontId::Regular, 12, rect(r.x + 6, r.y, r.w - 10, r.h), Align::Left, &name, BLACK);
            }
        }
        p.pop_clip();
        // the worksheet itself: white paper, light rules, figures set to the right
        p.push_clip(lay.grid);
        let (nr, nc) = (lay.rt.len() as i32, lay.ct.len() as i32);
        // the sheet is exactly as big as its categories — there is no grid beyond them
        p.fill(rect(lay.grid.x, lay.grid.y, nc * COL_W, nr * ROW_H), WHITE);
        for i in 0..=nc { p.vline(lay.grid.x + i * COL_W, lay.grid.y, nr * ROW_H, LIGHT); }
        for i in 0..=nr { p.hline(lay.grid.x, lay.grid.y + i * ROW_H, nc * COL_W, LIGHT); }
        let (sr, sc) = self.iv_sel_index(&lay);
        for r in 0..lay.rt.len() {
            let cell0 = lay.cell(r, 0);
            if cell0.y > lay.grid.bottom() { break; }
            for cc in 0..lay.ct.len() {
                let cell = lay.cell(r, cc);
                if cell.x > lay.grid.right() { break; }
                let editing = iv.edit.is_some() && (r, cc) == (sr, sc);
                let text = match (&iv.edit, editing) {
                    (Some(e), true) => e.clone(),
                    _ => iv.value(&lay.coord(iv.cats.len(), r, cc)).map(fmt).unwrap_or_default(),
                };
                if text.is_empty() { continue; }
                let w = p.text_width(FontId::Regular, 12, &text);
                let x = if editing { cell.x + 4 } else { cell.right() - 5 - w };
                p.text_in(FontId::Regular, 12, rect(x, cell.y, w, ROW_H), Align::Left, &text, BLACK);
                if editing { p.vline(x + w + 1, cell.y + 3, ROW_H - 6, BLACK); }
            }
        }
        let s = lay.cell(sr, sc);
        p.outline(s, BLACK); p.outline(s.inset(1), BLACK);
        p.pop_clip();
        // formula pane
        let pane = lay.pane;
        p.fill(pane, LIGHT);
        p.hline(pane.x, pane.y, pane.w, DARK); p.hline(pane.x, pane.y + 1, pane.w, WHITE);
        p.text(FontId::Bold, 12, pane.x + 8, pane.y + 16, "Formulas", BLACK);
        let list = self.iv_flist();
        p.fill(list, WHITE);
        p.bevel(list, DARK, WHITE);
        p.push_clip(list.inset(1));
        for i in 0..=iv.formulas.len() {
            let r = self.iv_frow(i);
            if r.bottom() > list.bottom() { break; }
            let sel = iv.fsel == Some(i);
            let text = if sel { iv.fedit.clone() } else { iv.formulas.get(i).cloned().unwrap_or_default() };
            if sel { p.fill(r, BLACK); }
            let fg = if sel { WHITE } else { BLACK };
            p.text_in(FontId::Regular, 12, rect(r.x + 4, r.y, r.w - 8, r.h), Align::Left, &text, fg);
            if sel { let w = p.text_width(FontId::Regular, 12, &text); p.vline(r.x + 5 + w, r.y + 2, r.h - 4, WHITE); }
        }
        p.pop_clip();
        if !iv.err.is_empty() {
            let msg = p.ellipsize(FontId::Regular, 12, &iv.err, pane.w - 16);
            p.text_in(FontId::Regular, 12, rect(pane.x + 8, pane.bottom() - 20, pane.w - 16, 16), Align::Left, &msg, DARK);
        }
        // the tile being dragged, over the slot it would land in
        if let Some(Capture::ImprovTile { cat, grab, at }) = &self.capture {
            let o = pt(at.x - grab.x, at.y - grab.y);
            let mid = o.x + TILE_W / 2;
            let (to_col, zone) = if mid >= lay.divider() { (true, lay.divider()) } else { (false, c.x) };
            let n = iv.axis(to_col).iter().filter(|i| *i != cat).count();
            let slot = (((mid - zone).max(0) / TILE_W) as usize).min(n) as i32;
            p.fill(rect(zone + slot * TILE_W, c.y + 1, 2, TILE_H - 4), BLACK);
            self.iv_tile(p, rect(o.x, o.y, TILE_W - 3, TILE_H - 4), &iv.cats[*cat].name);
        }
    }
    fn iv_tile(&self, p: &mut Painter, r: Rect, name: &str) {
        p.raised(r);
        let label = p.ellipsize(FontId::Bold, 12, name, r.w - 6);
        p.text_in(FontId::Bold, 12, r, Align::Center, &label, BLACK);
    }

    /// The visible grid as text, for the headless harness: `rowheads|colheads|v,v,…;v,v,…`.
    pub fn iv_dump(&self) -> String {
        let lay = self.iv_layout();
        let iv = &self.improv;
        let heads = |axis: &[usize], ts: &[Vec<u8>]| ts.iter()
            .map(|t| axis.iter().enumerate().map(|(i, &c)| iv.cats[c].items[t[i] as usize].clone()).collect::<Vec<_>>().join("/"))
            .collect::<Vec<_>>().join(",");
        let rows: Vec<String> = (0..lay.rt.len())
            .map(|r| (0..lay.ct.len()).map(|c| iv.value(&lay.coord(iv.cats.len(), r, c)).map(fmt).unwrap_or_default()).collect::<Vec<_>>().join(","))
            .collect();
        format!("{}|{}|{}", heads(&lay.rows, &lay.rt), heads(&lay.cols, &lay.ct), rows.join(";"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formulas_compute_over_every_cell_and_follow_edits() {
        let mut iv = Improv::default();
        let k = |p: u8, i: u8, q: u8| vec![p, i, q];
        assert_eq!(iv.value(&k(0, 2, 0)), Some(1500.0)); // Revenue = 120 × 12.50
        assert_eq!(iv.value(&k(0, 4, 0)), Some(600.0));  // Profit  = 1500 − 900, one pass later
        assert_eq!(iv.value(&k(1, 2, 3)), Some(3250.0)); // the other product, the last quarter
        iv.vals.insert(k(0, 0, 0), 200.0);               // type a new Units figure
        iv.recalc();
        assert_eq!(iv.value(&k(0, 2, 0)), Some(2500.0));
        assert_eq!(iv.value(&k(0, 4, 0)), Some(1600.0));
        iv.formulas[1] = "Profit = Revenue - Costs / 2".into(); // precedence, and a live edit
        iv.recalc();
        assert_eq!(iv.value(&k(0, 4, 0)), Some(2050.0));
        assert!(iv.err.is_empty());
        iv.formulas[0] = "Revenue = Units * Markup".into();
        iv.recalc();
        assert!(iv.err.contains("Markup"), "{}", iv.err);
    }

    #[test]
    fn pivoting_a_category_keeps_the_cube() {
        let mut iv = Improv::default();
        let before = iv.value(&[0, 2, 0]);
        assert_eq!(iv.axis(false).len(), 2);
        let quarters = iv.axis(true)[0];
        iv.move_cat(quarters, false, 0); // drag Quarters to the head of the row zone
        assert!(iv.axis(true).is_empty());
        assert_eq!(iv.axis(false), vec![0, 1, 2]);
        assert_eq!(iv.cats[0].name, "Quarters");
        assert_eq!(iv.value(&[0, 0, 2]), before); // same cell, permuted coordinates
        assert_eq!(iv.tuples(&iv.axis(false)).len(), 40);
    }
}
