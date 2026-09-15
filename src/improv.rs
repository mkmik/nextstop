//! Improv — after Lotus Improv (NeXT, 1991): a worksheet with no A1/B2 anywhere. Data lives in a
//! cube of named *categories*; formulas are written in plain English over the item names
//! ("Revenue = Units * Price") and apply to every cell of that item; the view is pivoted by
//! dragging the category tiles between the row zone and the column zone of the tile bar.
use crate::app::*;
use crate::backend::state;
use crate::chrome::*;
use crate::geom::{pt, rect, Pt, Rect};
use crate::paint::*;
use std::collections::HashMap;

const TILE_H: i32 = 21;   // the tile bar
const TILE_W: i32 = 86;   // a category tile — also one row-header column, so tiles sit over them
const HDR_H: i32 = 17;    // one column-header row (one per column category: nested headers)
pub const ROW_H: i32 = 16;
pub const COL_W: i32 = 74;
const FP_H: i32 = 96;     // formula pane: three rows, its smallest
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
    hedit: Option<(usize, usize, String)>, // a header being named: category, item, what is typed
    pub scroll: Pt,             // the sheet under the headers, in pixels
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
                cat("Quarters", &["Q1", "Q2", "Q3", "Q4", "Average"], true),
            ],
            vals: seed(), calc: HashMap::new(), targets: vec![],
            formulas: vec!["Revenue = Units * Price".into(), "Profit = Revenue - Costs".into(), "Average = Avg(Quarters)".into()],
            err: String::new(), sel: vec![0, 0, 0], edit: None, hedit: None, scroll: pt(0, 0), fsel: None, fedit: String::new(),
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

/// What a name stands for in one cell: an item is one figure, a category is all of its figures.
enum Ref { Cell(f64), Many(Vec<f64>) }
type Res<'a> = dyn Fn(&str) -> Option<Ref> + 'a;

struct Formula { target: (usize, usize), toks: Vec<Tok> }

/// The functions, over the figures their arguments came to. A category argument brings one figure
/// per item, so `Sum(Quarters)` adds up the row and `Max(Units, Price)` compares two cells.
// ponytail: arguments are flattened into one list, so If evaluates both of its branches — a guard
// like If(Units = 0, 0, Revenue / Units) still reports the division. Lazy branches need the
// evaluator to parse without evaluating, which is a different evaluator.
fn apply(name: &str, a: &[f64]) -> Result<f64, String> {
    let sum: f64 = a.iter().sum();
    let arity = |n: usize| if a.len() == n { Ok(()) } else { Err(format!("“{name}” takes {n} value(s), not {}", a.len())) };
    // over no figures at all these are not an error but nothing: the cell stays blank (see recalc)
    match name.to_ascii_lowercase().as_str() {
        "sum" | "total" => Ok(sum),
        "count" => Ok(a.len() as f64),
        "avg" | "average" | "mean" => Ok(sum / a.len() as f64),
        "min" => Ok(a.iter().copied().fold(f64::NAN, f64::min)),
        "max" => Ok(a.iter().copied().fold(f64::NAN, f64::max)),
        "abs" => { arity(1)?; Ok(a[0].abs()) }
        "int" => { arity(1)?; Ok(a[0].trunc()) }
        "sqrt" => { arity(1)?; if a[0] < 0.0 { Err("square root of a negative".into()) } else { Ok(a[0].sqrt()) } }
        "round" => match a.len() {
            1 => Ok(a[0].round()),
            2 => { let f = 10f64.powi(a[1] as i32); Ok((a[0] * f).round() / f) }
            n => Err(format!("“{name}” takes a figure and optionally the decimals, not {n} values")),
        },
        "if" => { arity(3)?; Ok(if a[0] != 0.0 { a[1] } else { a[2] }) }
        _ => Err(format!("no function named “{name}”")),
    }
}

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
        } else if "<>=".contains(c) {
            // “<=”, “>=” and “<>” become one character each, so the parser stays char-sized
            let (op, n) = match (c, ch.get(i + 1).copied().unwrap_or(' ')) { ('<', '=') => ('≤', 2), ('>', '=') => ('≥', 2), ('<', '>') => ('≠', 2), _ => (c, 1) };
            out.push(Tok::Op(op));
            i += n;
        } else if "+-*/(),".contains(c) { out.push(Tok::Op(c)); i += 1; }
        else { return Err(format!("unexpected “{c}”")); }
    }
    Ok(out)
}

struct Parse<'a> { t: &'a [Tok], i: usize }
impl Parse<'_> {
    fn op(&self, set: &str) -> Option<char> { match self.t.get(self.i) { Some(&Tok::Op(o)) if set.contains(o) => Some(o), _ => None } }
    /// The top of the grammar: a comparison, or just an expression. 1 and 0 stand for true and
    /// false, which is all `If` needs.
    fn cmp(&mut self, v: &Res) -> Result<f64, String> {
        let a = self.expr(v)?;
        let Some(o) = self.op("<>=≤≥≠") else { return Ok(a) };
        self.i += 1;
        let b = self.expr(v)?;
        let yes = match o { '<' => a < b, '>' => a > b, '=' => a == b, '≤' => a <= b, '≥' => a >= b, _ => a != b };
        Ok(if yes { 1.0 } else { 0.0 })
    }
    /// One argument: a bare category name brings all of its figures, anything else is one value.
    fn arg(&mut self, v: &Res) -> Result<Vec<f64>, String> {
        if let Some(Tok::Name(n)) = self.t.get(self.i).cloned() {
            if matches!(self.t.get(self.i + 1), Some(Tok::Op(',')) | Some(Tok::Op(')')) | None) {
                if let Some(Ref::Many(vs)) = v(&n) { self.i += 1; return Ok(vs); }
            }
        }
        Ok(vec![self.cmp(v)?])
    }
    fn call(&mut self, name: &str, v: &Res) -> Result<f64, String> {
        self.i += 1; // the “(”
        let mut a = vec![];
        while self.op(")").is_none() {
            a.extend(self.arg(v)?);
            if self.op(",").is_none() { break }
            self.i += 1;
        }
        if self.op(")").is_none() { return Err(format!("missing “)” after “{name}”")) }
        self.i += 1;
        apply(name, &a)
    }
    fn expr(&mut self, v: &Res) -> Result<f64, String> {
        let mut a = self.term(v)?;
        while let Some(o) = self.op("+-") { self.i += 1; let b = self.term(v)?; a = if o == '+' { a + b } else { a - b }; }
        Ok(a)
    }
    fn term(&mut self, v: &Res) -> Result<f64, String> {
        let mut a = self.factor(v)?;
        while let Some(o) = self.op("*/") {
            self.i += 1;
            let b = self.factor(v)?;
            if o == '/' && b == 0.0 { return Err("division by zero".into()); }
            a = if o == '*' { a * b } else { a / b };
        }
        Ok(a)
    }
    fn factor(&mut self, v: &Res) -> Result<f64, String> {
        if self.op("-").is_some() { self.i += 1; return Ok(-self.factor(v)?); }
        if self.op("+").is_some() { self.i += 1; return self.factor(v); }
        match self.t.get(self.i).cloned() {
            Some(Tok::Num(n)) => { self.i += 1; Ok(n) }
            Some(Tok::Name(n)) => {
                self.i += 1;
                if self.op("(").is_some() { return self.call(&n, v) }
                match v(&n) {
                    Some(Ref::Cell(x)) => Ok(x),
                    Some(Ref::Many(_)) => Err(format!("“{n}” is a category — use it inside Sum(…)")),
                    None => Err(format!("no item named “{n}”")),
                }
            }
            Some(Tok::Op('(')) => {
                self.i += 1;
                let r = self.cmp(v)?;
                if self.op(")").is_none() { return Err("missing “)”".into()); }
                self.i += 1;
                Ok(r)
            }
            _ => Err("expected a value".into()),
        }
    }
}

fn eval(toks: &[Tok], v: &Res) -> Result<f64, String> {
    let mut p = Parse { t: toks, i: 0 };
    let r = p.cmp(v)?;
    if p.i != toks.len() { return Err("extra text after the expression".into()); }
    Ok(r)
}

// ---- model ------------------------------------------------------------------------------------

impl Improv {
    /// The worksheet as `state.json` left it. An empty — or no longer self-consistent — one gives
    /// the sample worksheet back, the same way a fresh installation starts.
    pub fn from_sheet(s: &state::Sheet) -> Improv {
        let cats: Vec<Cat> = s.cats.iter().filter(|c| (1..=256).contains(&c.items.len())).map(|c| Cat { name: c.name.clone(), items: c.items.clone(), col: c.col }).collect();
        // an item index is one byte and every combination is a cell, so a hand-written file could
        // otherwise ask for a cube that does not fit in memory
        let cells = cats.iter().try_fold(1usize, |n, c| n.checked_mul(c.items.len()));
        if cats.is_empty() || cats.len() != s.cats.len() || cells.is_none_or(|n| n > 100_000) { return Improv::default() }
        // a hand-edited file can hold cells that no longer fit the categories; drop those
        let fits = |k: &Coord| k.len() == cats.len() && k.iter().zip(&cats).all(|(&i, c)| (i as usize) < c.items.len());
        let vals = s.cells.iter().filter(|(k, _)| fits(k)).map(|(k, v)| (k.clone(), *v)).collect();
        let mut iv = Improv {
            sel: vec![0; cats.len()], cats, vals, calc: HashMap::new(), targets: vec![],
            formulas: s.formulas.clone(), err: String::new(), edit: None, hedit: None, scroll: pt(0, 0), fsel: None, fedit: String::new(),
        };
        iv.recalc();
        iv
    }
    pub fn to_sheet(&self) -> state::Sheet {
        let mut cells: Vec<(Coord, f64)> = self.vals.iter().map(|(k, v)| (k.clone(), *v)).collect();
        cells.sort_by(|a, b| a.0.cmp(&b.0)); // a stable file, so a saved worksheet diffs cleanly
        state::Sheet {
            cats: self.cats.iter().map(|c| state::SheetCat { name: c.name.clone(), items: c.items.clone(), col: c.col }).collect(),
            cells,
            formulas: self.formulas.clone(),
        }
    }
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
    /// What `name` means for the cell at `k`: an item is the same cell with that category moved to
    /// that item (a blank reads as zero); a category is every one of its cells that holds a figure,
    /// minus the cell being computed — so `Average = Avg(Quarters)` does not eat its own tail.
    fn reference(&self, name: &str, k: &[u8], target: (usize, usize)) -> Option<Ref> {
        let mut k = k.to_vec();
        if let Some((c, i)) = self.find(name) {
            k[c] = i as u8;
            return Some(Ref::Cell(self.calc.get(&k).copied().unwrap_or(0.0)));
        }
        let c = self.cats.iter().position(|cat| cat.name.eq_ignore_ascii_case(name))?;
        Some(Ref::Many((0..self.cats[c].items.len()).filter(|&i| (c, i) != target)
            .filter_map(|i| { k[c] = i as u8; self.calc.get(&k).copied() }).collect()))
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
                    let r = eval(&f.toks, &|n| self.reference(n, k, f.target));
                    // a formula over nothing leaves the cell blank rather than writing a figure
                    match r { Ok(v) => if v.is_finite() { out.push((k.clone(), v)) }, Err(e) => if self.err.is_empty() { self.err = e } }
                }
                for (k, v) in out { self.calc.insert(k, v); }
            }
        }
    }

    /// The category a new row (or column) joins: the innermost one of that zone, which is the one
    /// whose items the view spends one line on.
    pub fn inner(&self, col: bool) -> Option<usize> { self.axis(col).last().copied() }
    /// Append an item to `cat`. An item index is one byte, which is the only ceiling.
    pub fn add_item(&mut self, cat: usize) -> Option<usize> {
        if self.cats[cat].items.len() >= 256 { return None }
        let name = (1..).map(|n| format!("Item {n}")).find(|n| !self.cats[cat].items.contains(n))?;
        self.cats[cat].items.push(name);
        Some(self.cats[cat].items.len() - 1)
    }
    /// Drop an item and its figures, sliding the ones after it down. The last item of a category
    /// cannot go: a category with no items has no cells at all.
    pub fn remove_item(&mut self, cat: usize, item: usize) {
        if self.cats[cat].items.len() < 2 || item >= self.cats[cat].items.len() { return }
        self.cats[cat].items.remove(item);
        self.vals = self.vals.iter().filter(|(k, _)| k[cat] as usize != item)
            .map(|(k, v)| { let mut k = k.clone(); if k[cat] as usize > item { k[cat] -= 1 } (k, *v) }).collect();
        self.sel[cat] = (self.sel[cat] as usize).min(self.cats[cat].items.len() - 1) as u8;
        self.recalc();
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
    pub sx: i32,            // left edge of the sheet: the vertical scroller has the strip before it
    pub off: Pt,            // how far the sheet is scrolled under its headers
    pub grid: Rect, pub pane: Rect, pub vbar: Rect, pub hbar: Rect,
}

impl Lay {
    pub fn divider(&self) -> i32 { self.sx + self.head_w }
    pub fn tile(&self, col: bool, i: usize) -> Rect {
        let x = if col { self.divider() } else { self.sx } + i as i32 * TILE_W;
        rect(x + 1, self.c.y + 2, TILE_W - 3, TILE_H - 4)
    }
    pub fn cell(&self, r: usize, c: usize) -> Rect { rect(self.grid.x - self.off.x + c as i32 * COL_W, self.grid.y - self.off.y + r as i32 * ROW_H, COL_W, ROW_H) }
    /// The row line and column line at `p`, which must be inside the grid.
    pub fn at(&self, p: Pt) -> (usize, usize) { (((p.y - self.grid.y + self.off.y) / ROW_H) as usize, ((p.x - self.grid.x + self.off.x) / COL_W) as usize) }
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
        // the pane is as tall as the formulas plus the empty row the next one is typed into,
        // up to half the window — otherwise a fourth formula has nowhere to be clicked
        let fp_h = (46 + (iv.formulas.len() as i32 + 1) * FROW_H).clamp(FP_H, (c.h / 2).max(FP_H));
        let pane = rect(c.x, c.bottom() - fp_h, c.w, fp_h);
        // NeXT puts the vertical scroller down the left edge; the horizontal one runs under the sheet
        let hbar = rect(c.x, pane.y - SCROLL_W, c.w, SCROLL_W);
        let vbar = rect(c.x, c.y + TILE_H, SCROLL_W, hbar.y - (c.y + TILE_H));
        let sx = c.x + SCROLL_W;
        let grid = rect(sx + head_w, c.y + head_h, c.right() - (sx + head_w), hbar.y - (c.y + head_h));
        let span = |n: usize, px: i32, vis: i32| (n as i32 * px - vis).max(0);
        let off = pt(iv.scroll.x.clamp(0, span(ct.len(), COL_W, grid.w)), iv.scroll.y.clamp(0, span(rt.len(), ROW_H, grid.h)));
        Lay { c, rows, cols, rt, ct, head_w, head_h, sx, off, grid, pane, vbar, hbar }
    }
    /// Whether the Item menu command applies: it needs a zone with a category in it, and the last
    /// item of a category cannot be deleted.
    pub fn iv_can(&self, a: Act) -> bool {
        let col = matches!(a, Act::IvNewCol | Act::IvDelCol);
        let Some(c) = self.improv.inner(col) else { return false };
        match a {
            Act::IvNewRow | Act::IvNewCol => self.improv.cats[c].items.len() < 256,
            _ => self.improv.cats[c].items.len() > 1,
        }
    }
    /// New Row / New Column: one more item in the innermost category of that zone, named on the
    /// spot — the header opens for typing, the way a new folder does in the File Viewer.
    pub fn iv_new_item(&mut self, col: bool) {
        self.iv_commit_formula();
        let Some(c) = self.improv.inner(col) else { return };
        let Some(i) = self.improv.add_item(c) else { return self.log("a category holds at most 256 items".into()) };
        self.improv.sel[c] = i as u8;
        self.improv.hedit = Some((c, i, String::new()));
        self.improv.recalc();
        self.iv_edited();
    }
    /// Delete Row / Delete Column: the item the selection sits in, and its figures with it.
    pub fn iv_del_item(&mut self, col: bool) {
        let Some(c) = self.improv.inner(col) else { return };
        let (item, name) = (self.improv.sel[c] as usize, self.iv_item_name(c));
        self.improv.hedit = None;
        self.improv.remove_item(c, item);
        self.log(format!("deleted “{name}”"));
        self.iv_edited();
    }
    fn iv_item_name(&self, c: usize) -> String { self.improv.cats[c].items[self.improv.sel[c] as usize].clone() }
    /// A name typed into a header. An empty one leaves the item as it was.
    fn iv_commit_name(&mut self) {
        let Some((c, i, text)) = self.improv.hedit.take() else { return };
        let name = text.trim().to_string();
        if name.is_empty() || self.improv.cats[c].items.iter().enumerate().any(|(j, n)| j != i && *n == name) { return }
        if name.contains(char::is_whitespace) { self.log(format!("“{name}” has a space, so a formula cannot refer to it")); }
        self.improv.cats[c].items[i] = name;
        self.improv.recalc();
        self.iv_edited();
    }
    /// Every edit goes straight into `state.json`, which is the worksheet's document.
    fn iv_edited(&mut self) {
        self.state.improv = self.improv.to_sheet();
        self.dirty();
    }
    /// The sheet's two scrollers: down the left edge and along the bottom.
    pub fn iv_scrollers(&self) -> (Scroller, Scroller) {
        let lay = self.iv_layout();
        (Scroller::framed(lay.vbar, true, lay.rt.len() as i32 * ROW_H, lay.grid.h, lay.off.y),
         Scroller::framed(lay.hbar, false, lay.ct.len() as i32 * COL_W, lay.grid.w, lay.off.x))
    }
    pub fn iv_btn_hit(&self, p: Pt) -> Option<Btn> {
        let (v, h) = self.iv_scrollers();
        [(ScrollId::Sheet, v), (ScrollId::SheetH, h)].into_iter().find_map(|(id, sc)| match sc.hit(p) {
            Some(ScrollHit::ArrowA) => Some(Btn::ScrollArrow(id, -1)),
            Some(ScrollHit::ArrowB) => Some(Btn::ScrollArrow(id, 1)),
            _ => None,
        })
    }
    /// Bring the selected cell inside the grid, the way arrow keys do in any sheet.
    fn iv_reveal(&mut self) {
        let lay = self.iv_layout();
        let (r, c) = self.iv_sel_index(&lay);
        let axis = |pos: i32, at: i32, cell: i32, vis: i32| { let lo = (at + cell - vis).max(0); pos.clamp(lo, at.max(lo)) };
        self.improv.scroll = pt(axis(lay.off.x, c as i32 * COL_W, COL_W, lay.grid.w), axis(lay.off.y, r as i32 * ROW_H, ROW_H, lay.grid.h));
    }
    fn iv_flist(&self) -> Rect { let p = self.iv_layout().pane; rect(p.x + 8, p.y + 20, p.w - 16, p.h - 44) }
    fn iv_frow(&self, i: usize) -> Rect { let l = self.iv_flist(); rect(l.x + 2, l.y + 2 + i as i32 * FROW_H, l.w - 4, FROW_H) }

    // ---- interaction --------------------------------------------------------------------------

    pub fn iv_mouse_down(&mut self, p: Pt) {
        let (v, h) = self.iv_scrollers();
        if self.scroller_down(ScrollId::Sheet, v, p) || self.scroller_down(ScrollId::SheetH, h, p) { return }
        let lay = self.iv_layout();
        for col in [false, true] {
            let axis = self.improv.axis(col);
            if let Some(i) = (0..axis.len()).find(|&i| lay.tile(col, i).contains(p)) {
                let r = lay.tile(col, i);
                self.iv_commit_name();
                self.iv_commit_formula();
                self.capture = Some(Capture::ImprovTile { cat: axis[i], grab: pt(p.x - r.x, p.y - r.y), at: p });
                return;
            }
        }
        if let Some((c, i)) = self.iv_header_hit(&lay, p) {
            self.iv_commit_name();
            self.iv_commit_formula();
            self.iv_commit_cell();
            // a click picks the row or column; a click on the one already picked names it, as a
            // second click on a File Viewer icon opens its name for editing
            if self.improv.sel[c] as usize == i { self.improv.hedit = Some((c, i, self.improv.cats[c].items[i].clone())); }
            else { self.improv.sel[c] = i as u8; }
            return;
        }
        self.iv_commit_name();
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
            let (r, c) = lay.at(p);
            if r < lay.rt.len() && c < lay.ct.len() {
                self.iv_commit_cell();
                self.improv.sel = lay.coord(self.improv.cats.len(), r, c);
            }
        }
    }

    /// The (category, item) of the row or column header under `p`.
    fn iv_header_hit(&self, lay: &Lay, p: Pt) -> Option<(usize, usize)> {
        let heads = rect(lay.sx, lay.c.y + TILE_H, lay.head_w, lay.head_h - TILE_H);
        if (lay.sx..lay.grid.x).contains(&p.x) && lay.grid.contains(pt(lay.grid.x, p.y)) {
            let l = ((p.x - lay.sx) / TILE_W) as usize;
            let (r, _) = lay.at(pt(lay.grid.x, p.y));
            return Some((*lay.rows.get(l)?, *lay.rt.get(r)?.get(l)? as usize));
        }
        if heads.y <= p.y && p.y < heads.bottom() && p.x >= lay.grid.x && p.x < lay.grid.right() {
            let l = ((p.y - heads.y) / HDR_H) as usize;
            let (_, c) = lay.at(pt(p.x, lay.grid.y));
            return Some((*lay.cols.get(l)?, *lay.ct.get(c)?.get(l)? as usize));
        }
        None
    }

    pub fn iv_tile_drop(&mut self, cat: usize, origin: Pt) {
        let lay = self.iv_layout();
        let mid = origin.x + TILE_W / 2;
        let to_col = mid >= lay.divider();
        let zone = if to_col { lay.divider() } else { lay.sx };
        self.improv.move_cat(cat, to_col, ((mid - zone).max(0) / TILE_W) as usize);
        self.iv_edited();
    }

    pub fn iv_key(&mut self, k: Key, _mods: Mods) {
        if let Some((c, i, mut t)) = self.improv.hedit.take() {
            match k {
                Key::Char(ch) => t.push(ch),
                Key::Backspace | Key::Delete => { t.pop(); }
                Key::Escape => return,                              // the item keeps the name it had
                Key::Enter | Key::Tab => { self.improv.hedit = Some((c, i, t)); return self.iv_commit_name() }
                _ => {}
            }
            self.improv.hedit = Some((c, i, t));
            return;
        }
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
                if !self.improv.computed(&sel) { self.improv.vals.remove(&sel); self.improv.recalc(); self.iv_edited(); }
            }
            Key::Enter | Key::Down => { self.iv_commit_cell(); r = (r + 1).min(last.0); }
            Key::Up => { self.iv_commit_cell(); r = r.saturating_sub(1); }
            Key::Left => { self.iv_commit_cell(); c = c.saturating_sub(1); }
            Key::Right | Key::Tab => { self.iv_commit_cell(); c = (c + 1).min(last.1); }
            _ => {}
        }
        self.improv.sel = lay.coord(self.improv.cats.len(), r, c);
        self.iv_reveal();
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
        self.iv_edited();
    }
    fn iv_commit_formula(&mut self) {
        let Some(i) = self.improv.fsel.take() else { return };
        let text = std::mem::take(&mut self.improv.fedit).trim().to_string();
        if i < self.improv.formulas.len() {
            if text.is_empty() { self.improv.formulas.remove(i); } else { self.improv.formulas[i] = text; }
        } else if !text.is_empty() { self.improv.formulas.push(text); }
        self.improv.recalc();
        self.iv_edited();
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
        let corner = rect(lay.sx, c.y + TILE_H, lay.head_w, lay.head_h - TILE_H);
        p.fill(corner, LIGHT); p.bevel(corner, WHITE, DARK);
        p.push_clip(rect(lay.grid.x, corner.y, c.right() - lay.grid.x, corner.h));
        for (l, &cat) in lay.cols.iter().enumerate() {
            let span = iv.span(&lay.cols, l);
            for (i, t) in lay.ct.iter().enumerate().step_by(span) {
                let r = rect(lay.grid.x - lay.off.x + i as i32 * COL_W, corner.y + l as i32 * HDR_H, span as i32 * COL_W, HDR_H);
                self.iv_head(p, r, cat, t[l] as usize, Align::Center);
            }
        }
        p.pop_clip();
        p.push_clip(rect(lay.sx, lay.grid.y, lay.head_w, lay.grid.h));
        for (l, &cat) in lay.rows.iter().enumerate() {
            let span = iv.span(&lay.rows, l);
            for (i, t) in lay.rt.iter().enumerate().step_by(span) {
                let r = rect(lay.sx + l as i32 * TILE_W, lay.grid.y - lay.off.y + i as i32 * ROW_H, TILE_W, span as i32 * ROW_H);
                self.iv_head(p, r, cat, t[l] as usize, Align::Left);
            }
        }
        p.pop_clip();
        // the worksheet itself: white paper, light rules, figures set to the right
        p.push_clip(lay.grid);
        let (nr, nc) = (lay.rt.len() as i32, lay.ct.len() as i32);
        // the sheet is exactly as big as its categories — there is no grid beyond them
        let (ox, oy) = (lay.grid.x - lay.off.x, lay.grid.y - lay.off.y);
        p.fill(rect(ox, oy, nc * COL_W, nr * ROW_H), WHITE);
        for i in 0..=nc { p.vline(ox + i * COL_W, oy, nr * ROW_H, LIGHT); }
        for i in 0..=nr { p.hline(ox, oy + i * ROW_H, nc * COL_W, LIGHT); }
        let (sr, sc) = self.iv_sel_index(&lay);
        for r in 0..lay.rt.len() {
            let cell0 = lay.cell(r, 0);
            if cell0.y > lay.grid.bottom() { break; }
            if cell0.bottom() < lay.grid.y { continue; }
            for cc in 0..lay.ct.len() {
                let cell = lay.cell(r, cc);
                if cell.x > lay.grid.right() { break; }
                if cell.right() < lay.grid.x { continue; }
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
        let (v, h) = self.iv_scrollers();
        let arrow = |id: ScrollId| self.pressed().and_then(|b| match b {
            Btn::ScrollArrow(i, d) if i == id => Some(if d < 0 { ScrollHit::ArrowA } else { ScrollHit::ArrowB }),
            _ => None,
        });
        v.draw(p, arrow(ScrollId::Sheet));
        h.draw(p, arrow(ScrollId::SheetH));
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
    /// One row or column header: the item's name, or what is being typed into it.
    fn iv_head(&self, p: &mut Painter, r: Rect, cat: usize, item: usize, align: Align) {
        let typing = match &self.improv.hedit { Some((c, i, t)) if (*c, *i) == (cat, item) => Some(t.clone()), _ => None };
        p.fill(r, if typing.is_some() { WHITE } else { LIGHT });
        p.bevel(r, if typing.is_some() { DARK } else { WHITE }, if typing.is_some() { WHITE } else { DARK });
        let pad = if align == Align::Left { 6 } else { 2 };
        let inner = rect(r.x + pad, r.y, r.w - 2 * pad, r.h);
        let text = p.ellipsize(FontId::Regular, 12, typing.as_ref().unwrap_or(&self.improv.cats[cat].items[item]), inner.w);
        p.text_in(FontId::Regular, 12, inner, align, &text, BLACK);
        if typing.is_some() {
            let w = p.text_width(FontId::Regular, 12, &text);
            let x = if align == Align::Center { inner.x + (inner.w + w) / 2 } else { inner.x + w };
            p.vline(x + 1, r.y + 3, r.h - 6, BLACK);
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
    fn functions_aggregate_over_a_category_without_eating_their_own_cell() {
        let mut iv = Improv::default();
        let k = |p: u8, i: u8, q: u8| vec![p, i, q];
        // the sample's third formula, Average = Avg(Quarters), over every item of every product
        assert_eq!(iv.value(&k(0, 0, 4)), Some(158.75));   // Units:   (120+145+160+210)/4
        assert_eq!(iv.value(&k(0, 2, 4)), Some(2030.625)); // Revenue: the four computed ones
        iv.formulas[2] = "Average = Sum(Quarters)".into();
        iv.recalc();
        assert_eq!(iv.value(&k(0, 0, 4)), Some(635.0), "{}", iv.err);
        iv.recalc();
        assert_eq!(iv.value(&k(0, 0, 4)), Some(635.0), "a second pass must not add the cell to itself");
        for (expr, want) in [
            ("Min(Quarters)", 120.0), ("Max(Quarters)", 210.0), ("Count(Quarters)", 4.0),
            ("Sum(Quarters) / Count(Quarters)", 158.75), ("Round(Avg(Quarters), 1)", 158.8),
            ("If(Max(Quarters) >= 300, 1, 0)", 0.0), ("If(Max(Quarters) <> 210, 1, 0)", 0.0),
            ("Sqrt(16) + Abs(0 - 7)", 11.0), ("Max(Units, Price, 200)", 200.0),
        ] {
            iv.formulas[2] = format!("Average = {expr}");
            iv.recalc();
            assert_eq!(iv.value(&k(0, 0, 4)), Some(want), "{expr} — {}", iv.err);
        }
        iv.vals.remove(&k(0, 0, 1)); // a blank quarter is not a zero
        iv.formulas[2] = "Average = Count(Quarters)".into();
        iv.recalc();
        assert_eq!(iv.value(&k(0, 0, 4)), Some(3.0));
        iv.formulas[2] = "Average = Min(Quarters)".into();
        iv.vals.retain(|k, _| k[1] != 0); // every Units figure gone: nothing to take the minimum of
        iv.recalc();
        assert_eq!(iv.value(&k(0, 0, 4)), None, "a formula over no figures leaves the cell blank");
        assert!(iv.err.is_empty(), "and says nothing: {}", iv.err);
        for (expr, want) in [("Avg(Missing)", "Missing"), ("Quarters * 2", "category"), ("Sum(Quarters", "missing")] {
            iv.formulas[2] = format!("Average = {expr}");
            iv.recalc();
            assert!(iv.err.contains(want), "{expr} — {}", iv.err);
        }
    }

    #[test]
    fn the_worksheet_is_written_to_the_state_and_read_back() {
        let mut a = App::test_app();
        a.show_win(WinKind::Improv);
        for c in "250".chars() { a.iv_key(Key::Char(c), Mods::default()); } // into Widgets/Units/Q1
        a.iv_key(Key::Enter, Mods::default());
        a.improv.fsel = Some(3);
        a.improv.fedit = "Costs = Units * 4".into();
        a.iv_commit_formula();
        let items = a.improv.axis(false)[1];
        a.iv_tile_drop(items, pt(10_000, 0)); // drag Items over to the column zone
        let saved = a.state.improv.clone();
        assert!(saved.cells.contains(&(vec![0, 0, 0], 250.0)), "the figure reached state.json");
        assert_eq!(saved.cats.iter().map(|c| c.col).collect::<Vec<_>>(), vec![false, true, true]);

        let back = Improv::from_sheet(&saved); // what the next launch sees
        assert_eq!(back.formulas, a.improv.formulas);
        assert_eq!(back.axis(true).len(), 2);
        assert_eq!(back.value(&[0, 0, 2]), Some(3125.0), "Widgets/Q1/Revenue = 250 × 12.50, recomputed on load");
        assert_eq!(back.value(&[0, 0, 3]), Some(1000.0), "the edited formula came back too");

        let sheet = |items: Vec<String>| state::Sheet { cats: vec![state::SheetCat { name: "Big".into(), items, col: false }], cells: vec![(vec![9], 1.0)], formulas: vec![] };
        for bad in [vec![], (0..300).map(|i| i.to_string()).collect::<Vec<_>>()] {
            assert_eq!(Improv::from_sheet(&sheet(bad)).cats.len(), 3, "an unusable worksheet falls back to the sample");
        }
    }

    #[test]
    fn a_row_is_added_named_in_its_header_and_deleted_with_its_figures() {
        let mut a = App::test_app();
        a.show_win(WinKind::Improv);
        let main = |a: &App| a.menus.iter().find(|m| m.kind == MenuKind::Main).unwrap().title.clone();
        assert_eq!(main(&a), "Improv", "the key window owns the main menu");
        let items = a.improv.inner(false).unwrap(); // the innermost row category
        a.iv_new_item(false);
        let n = a.improv.cats[items].items.len();
        assert!(a.improv.hedit.is_some(), "the new row's header is open for typing");
        for c in "Margin".chars() { a.iv_key(Key::Char(c), Mods::default()); }
        a.iv_key(Key::Enter, Mods::default());
        assert_eq!(a.improv.cats[items].items[n - 1], "Margin");

        a.improv.fsel = Some(a.improv.formulas.len()); // a formula for the row just made
        a.improv.fedit = "Margin = Round(Profit / Revenue * 100, 1)".into();
        a.iv_commit_formula();
        assert_eq!(a.improv.value(&[0, 5, 0]), Some(40.0), "{}", a.improv.err);

        // a click picks a row, a second click on the same one opens its name for editing
        let lay = a.iv_layout();
        let price = pt(lay.sx + TILE_W + 4, lay.grid.y + ROW_H + 4);
        a.iv_mouse_down(price);
        assert_eq!((a.improv.sel[items], a.improv.hedit.is_some()), (1, false));
        a.iv_mouse_down(price);
        assert_eq!(a.improv.hedit.as_ref().map(|(c, i, t)| (*c, *i, t.clone())), Some((items, 1, "Price".to_string())));
        a.iv_key(Key::Escape, Mods::default());
        a.improv.sel[items] = (n - 1) as u8;

        a.iv_del_item(false); // Delete Row takes the row the selection is in
        assert_eq!(a.improv.cats[items].items.len(), n - 1);
        assert_eq!(a.improv.value(&[0, 4, 0]), Some(600.0), "the rows above keep their figures");
        assert!(a.state.improv.cats[items].items.iter().all(|i| i != "Margin"), "and state.json followed");

        a.close_win(WinKind::Improv);
        assert_eq!(main(&a), "Workspace", "the menu goes back when the window does");
    }

    #[test]
    fn the_sheet_scrolls_on_both_axes_and_the_selection_stays_in_view() {
        let mut a = App::test_app();
        a.show_win(WinKind::Improv);
        for _ in 0..20 { a.iv_new_item(false); a.iv_key(Key::Escape, Mods::default()); }  // rows
        for _ in 0..10 { a.iv_new_item(true); a.iv_key(Key::Escape, Mods::default()); }   // columns
        let lay = a.iv_layout();
        assert!(lay.rt.len() as i32 * ROW_H > lay.grid.h && lay.ct.len() as i32 * COL_W > lay.grid.w);
        let (v, h) = a.iv_scrollers();
        assert!(!v.fits() && !h.fits() && (v.pos, h.pos) == (0, 0));

        for _ in 0..lay.rt.len() { a.iv_key(Key::Down, Mods::default()); }
        for _ in 0..lay.ct.len() { a.iv_key(Key::Right, Mods::default()); }
        let (v, h) = a.iv_scrollers();
        assert!(v.pos > 0 && v.pos == v.max_pos(), "the last row brought itself into view");
        assert!(h.pos > 0 && h.pos == h.max_pos(), "and so did the last column");

        a.set_scroll(ScrollId::Sheet, 0);                 // the wheel and the arrows come through here
        assert_eq!(a.iv_scrollers().0.pos, 0);
        a.scroll_by(ScrollId::SheetH, 10_000);
        assert_eq!(a.iv_scrollers().1.pos, h.max_pos(), "and never past the end");

        // a header click still lands on the right item once the sheet is scrolled
        let lay = a.iv_layout();
        let first = lay.at(pt(lay.grid.x + 4, lay.grid.y + 4)).1;
        assert!(first > 0, "the leftmost visible column is no longer the first one");
        let hit = a.iv_header_hit(&lay, pt(lay.grid.x + 4, lay.c.y + TILE_H + 4));
        assert_eq!(hit, Some((lay.cols[0], lay.ct[first][0] as usize)));
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
        assert_eq!(iv.tuples(&iv.axis(false)).len(), 50);
    }
}
