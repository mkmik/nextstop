//! Software painter: everything is drawn in logical NeXT pixels and scaled to device pixels here.
use crate::geom::{rect, Rect};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub const BLACK: u32 = 0x000000;
pub const DARK: u32 = 0x555555;
pub const LIGHT: u32 = 0xaaaaaa;
pub const WHITE: u32 = 0xffffff;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum FontId { Regular, Bold, Mono }

pub struct Glyph { pub metrics: fontdue::Metrics, pub bitmap: Vec<u8> }

pub struct Fonts {
    faces: [fontdue::Font; 3],
    cache: RefCell<HashMap<(FontId, u32, char), Rc<Glyph>>>,
}

impl Fonts {
    pub fn load() -> Fonts {
        let f = |b: &[u8]| fontdue::Font::from_bytes(b, fontdue::FontSettings::default()).expect("font");
        Fonts {
            faces: [
                f(include_bytes!("../assets/fonts/LiberationSans-Regular.ttf")),
                f(include_bytes!("../assets/fonts/LiberationSans-Bold.ttf")),
                f(include_bytes!("../assets/fonts/LiberationMono-Regular.ttf")),
            ],
            cache: RefCell::new(HashMap::new()),
        }
    }
    fn face(&self, id: FontId) -> &fontdue::Font { &self.faces[id as usize] }
    fn glyph(&self, id: FontId, px: u32, ch: char) -> Rc<Glyph> {
        if let Some(g) = self.cache.borrow().get(&(id, px, ch)) { return g.clone(); }
        let (metrics, bitmap) = self.face(id).rasterize(ch, px as f32);
        let g = Rc::new(Glyph { metrics, bitmap });
        self.cache.borrow_mut().insert((id, px, ch), g.clone());
        g
    }
    /// Whether the face can draw this character at all; the bundled Liberation faces stop well
    /// short of Unicode (no ▸, for one), and a missing glyph is drawn as an empty box.
    #[cfg(test)]
    pub fn has_glyph(&self, id: FontId, ch: char) -> bool { self.face(id).lookup_glyph_index(ch) != 0 }
    /// Advance width in device pixels.
    pub fn width_px(&self, id: FontId, px: u32, text: &str) -> f32 {
        text.chars().map(|c| self.face(id).metrics(c, px as f32).advance_width).sum()
    }
    pub fn line(&self, id: FontId, px: u32) -> fontdue::LineMetrics {
        self.face(id).horizontal_line_metrics(px as f32).expect("line metrics")
    }
}

/// Premultiplied RGBA image (device pixels).
pub struct Image { pub w: u32, pub h: u32, pub px: Vec<[u8; 4]> }

pub struct Icons {
    trees: HashMap<&'static str, resvg::usvg::Tree>,
    cache: RefCell<HashMap<(String, u32), Rc<Image>>>,
}

macro_rules! icon_set {
    ($($n:literal),* $(,)?) => { [$(($n, include_str!(concat!("../assets/icons/", $n, ".svg")))),*] };
}
const SVGS: [(&str, &str); 25] = icon_set![
    "alert", "application", "arrow", "computer", "drive-net", "drive", "file-archive", "file-audio", "file-code", "file-generic",
    "file-image", "file-pdf", "file-text", "file-video", "folder-open", "folder", "home", "miniwindow", "recycler-empty",
    "recycler-full", "resize-nesw", "resize-ns", "resize-nwse", "symlink-badge", "workspace",
];

impl Icons {
    pub fn load() -> Icons {
        let opt = resvg::usvg::Options::default();
        let trees = SVGS.iter().map(|(n, s)| (*n, resvg::usvg::Tree::from_str(s, &opt).expect(n))).collect();
        Icons { trees, cache: RefCell::new(HashMap::new()) }
    }
    /// Rasterize `name` so that its longer side is `px` device pixels.
    pub fn get(&self, name: &str, px: u32) -> Rc<Image> {
        let key = (name.to_string(), px);
        if let Some(i) = self.cache.borrow().get(&key) { return i.clone(); }
        let tree = self.trees.get(name).unwrap_or_else(|| &self.trees["file-generic"]);
        let size = tree.size();
        let k = px as f32 / size.width().max(size.height());
        let (w, h) = ((size.width() * k).round().max(1.0) as u32, (size.height() * k).round().max(1.0) as u32);
        let mut pm = resvg::tiny_skia::Pixmap::new(w, h).unwrap();
        resvg::render(tree, resvg::tiny_skia::Transform::from_scale(k, k), &mut pm.as_mut());
        let px_data = pm.data().chunks_exact(4).map(|c| [c[0], c[1], c[2], c[3]]).collect();
        let img = Rc::new(Image { w, h, px: px_data });
        self.cache.borrow_mut().insert(key, img.clone());
        img
    }
    /// Non-premultiplied RGBA bytes (for OS cursors).
    pub fn rgba(&self, name: &str, px: u32) -> (u32, u32, Vec<u8>) {
        let i = self.get(name, px);
        let mut out = Vec::with_capacity(i.px.len() * 4);
        for p in &i.px {
            let a = p[3] as u32;
            let un = |c: u8| (c as u32 * 255 + a / 2).checked_div(a).map_or(0, |v| v.min(255) as u8);
            out.extend_from_slice(&[un(p[0]), un(p[1]), un(p[2]), p[3]]);
        }
        (i.w, i.h, out)
    }
}

/// Decode a PNG into a premultiplied image.
pub fn decode_png(bytes: &[u8]) -> Result<Image, String> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
    dec.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = dec.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0; reader.output_buffer_size().unwrap_or(0)];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let bytes = &buf[..info.buffer_size()];
    let n = match info.color_type { png::ColorType::Rgba => 4, png::ColorType::Rgb => 3, _ => return Err("unsupported PNG color type".into()) };
    let px = bytes.chunks_exact(n).map(|c| {
        let a = if n == 4 { c[3] as u32 } else { 255 };
        let pm = |v: u8| ((v as u32 * a + 127) / 255) as u8;
        [pm(c[0]), pm(c[1]), pm(c[2]), a as u8]
    }).collect();
    Ok(Image { w: info.width, h: info.height, px })
}

pub struct Frame<'a> { pub w: u32, pub h: u32, pub px: &'a mut [u32] }

pub struct Painter<'a> {
    pub fb: Frame<'a>,
    pub s: f32,
    clip: Vec<Rect>, // device-space clip rects
    pub fonts: &'a Fonts,
    pub icons: &'a Icons,
    ox: i32, oy: i32, // logical origin of this surface (Screen coordinates of its top-left)
    slant: f32,       // glyph lean per pixel above the baseline (text_oblique)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Align { Left, Center }

impl<'a> Painter<'a> {
    pub fn new(fb: Frame<'a>, s: f32, fonts: &'a Fonts, icons: &'a Icons) -> Painter<'a> {
        let clip = vec![rect(0, 0, fb.w as i32, fb.h as i32)];
        Painter { fb, s, clip, fonts, icons, ox: 0, oy: 0, slant: 0.0 }
    }
    /// Draw Screen-coordinate content into a surface whose top-left is at logical (ox, oy).
    pub fn set_origin(&mut self, ox: i32, oy: i32) { self.ox = ox; self.oy = oy; }
    /// Device-pixel size for a logical font size.
    pub fn px(&self, logical: i32) -> u32 { (logical as f32 * self.s).round().max(1.0) as u32 }
    fn d(&self, v: i32) -> i32 { (v as f32 * self.s).round() as i32 }
    fn dx(&self, x: i32) -> i32 { ((x - self.ox) as f32 * self.s).round() as i32 }
    fn dy(&self, y: i32) -> i32 { ((y - self.oy) as f32 * self.s).round() as i32 }
    fn dev(&self, r: Rect) -> Rect {
        let x0 = self.dx(r.x); let y0 = self.dy(r.y);
        rect(x0, y0, self.dx(r.right()) - x0, self.dy(r.bottom()) - y0)
    }
    pub fn push_clip(&mut self, r: Rect) { let c = self.dev(r).intersect(*self.clip.last().unwrap()); self.clip.push(c); }
    pub fn pop_clip(&mut self) { if self.clip.len() > 1 { self.clip.pop(); } }
    fn clipd(&self) -> Rect { *self.clip.last().unwrap() }

    fn fill_dev(&mut self, r: Rect, color: u32) {
        let r = r.intersect(self.clipd());
        if r.is_empty() { return; }
        let w = self.fb.w as usize;
        for y in r.y..r.bottom() {
            let row = &mut self.fb.px[y as usize * w + r.x as usize..y as usize * w + r.right() as usize];
            row.fill(color);
        }
    }
    pub fn fill(&mut self, r: Rect, color: u32) { let d = self.dev(r); self.fill_dev(d, color); }
    pub fn hline(&mut self, x: i32, y: i32, w: i32, color: u32) { self.fill(rect(x, y, w, 1), color); }
    pub fn vline(&mut self, x: i32, y: i32, h: i32, color: u32) { self.fill(rect(x, y, 1, h), color); }
    pub fn outline(&mut self, r: Rect, color: u32) {
        self.hline(r.x, r.y, r.w, color); self.hline(r.x, r.bottom() - 1, r.w, color);
        self.vline(r.x, r.y, r.h, color); self.vline(r.right() - 1, r.y, r.h, color);
    }
    /// Bevel edges: top/left `tl`, bottom/right `br`, both 1 px inside `r`.
    pub fn bevel(&mut self, r: Rect, tl: u32, br: u32) {
        self.hline(r.x, r.y, r.w, tl); self.vline(r.x, r.y, r.h, tl);
        self.hline(r.x + 1, r.bottom() - 1, r.w - 1, br); self.vline(r.right() - 1, r.y + 1, r.h - 1, br);
    }
    /// Raised control (§6.2): light face, white top/left, dark bottom/right, black outer shadow bottom/right.
    pub fn raised(&mut self, r: Rect) { self.fill(r, LIGHT); self.bevel(r, WHITE, DARK); self.shadow(r); }
    /// Pressed raised control: sunken edges but keeps the outer shadow.
    pub fn pressed(&mut self, r: Rect) { self.fill(r, LIGHT); self.bevel(r, DARK, WHITE); self.shadow(r); }
    pub fn shadow(&mut self, r: Rect) { self.hline(r.x + 1, r.bottom(), r.w, BLACK); self.vline(r.right(), r.y + 1, r.h, BLACK); }
    pub fn sunken(&mut self, r: Rect) { self.fill(r, LIGHT); self.bevel(r, DARK, WHITE); }
    /// 50 % checkerboard of light and dark gray, one logical pixel per cell (NeXT scroller tracks).
    pub fn dither(&mut self, r: Rect) {
        self.fill(r, LIGHT);
        for y in r.y..r.bottom() {
            let start = if (r.x + y) & 1 == 0 { r.x } else { r.x + 1 };
            let mut x = start;
            while x < r.right() { self.fill(rect(x, y, 1, 1), DARK); x += 2; }
        }
    }

    /// Blend a premultiplied image at logical (x, y); it is drawn at its own device size, scaled to `w`×`h` logical if given.
    pub fn image(&mut self, img: &Image, x: i32, y: i32, size: Option<(i32, i32)>, opacity: u8) {
        let (dw, dh) = match size { Some((w, h)) => (self.d(w), self.d(h)), None => (img.w as i32, img.h as i32) };
        let dst = rect(self.dx(x), self.dy(y), dw, dh);
        let vis = dst.intersect(self.clipd());
        if vis.is_empty() || img.w == 0 || img.h == 0 { return; }
        let fw = self.fb.w as usize;
        for yy in vis.y..vis.bottom() {
            let sy = ((yy - dst.y) as u32 * img.h / dh.max(1) as u32).min(img.h - 1);
            for xx in vis.x..vis.right() {
                let sx = ((xx - dst.x) as u32 * img.w / dw.max(1) as u32).min(img.w - 1);
                let p = img.px[(sy * img.w + sx) as usize];
                if p[3] == 0 { continue; }
                let i = yy as usize * fw + xx as usize;
                let (sr, sg, sb, sa) = ((p[0] as u32 * opacity as u32) / 255, (p[1] as u32 * opacity as u32) / 255, (p[2] as u32 * opacity as u32) / 255, (p[3] as u32 * opacity as u32) / 255);
                let d = self.fb.px[i];
                let (dr, dg, db) = ((d >> 16) & 255, (d >> 8) & 255, d & 255);
                let inv = 255 - sa;
                let r = (sr + dr * inv / 255).min(255); let g = (sg + dg * inv / 255).min(255); let b = (sb + db * inv / 255).min(255);
                self.fb.px[i] = (r << 16) | (g << 8) | b;
            }
        }
    }
    pub fn icon(&mut self, name: &str, x: i32, y: i32, size: i32) {
        let img = self.icons.get(name, self.px(size));
        self.image(&img, x, y, Some((size, size)), 255);
    }

    // ---- text
    pub fn text_width(&self, font: FontId, size: i32, text: &str) -> i32 {
        (self.fonts.width_px(font, self.px(size), text) / self.s).round() as i32
    }
    /// Draw `text` with its baseline at logical `y_base`. Returns the advance in logical px.
    pub fn text(&mut self, font: FontId, size: i32, x: i32, y_base: i32, text: &str, color: u32) -> i32 {
        let px = self.px(size);
        let mut pen = self.dx(x) as f32;
        let base = self.dy(y_base);
        let clip = self.clipd();
        let fw = self.fb.w as usize;
        let (cr, cg, cb) = ((color >> 16) & 255, (color >> 8) & 255, color & 255);
        for ch in text.chars() {
            let g = self.fonts.glyph(font, px, ch);
            let m = &g.metrics;
            let gx = pen.round() as i32 + m.xmin;
            let gy = base - m.ymin - m.height as i32;
            for row in 0..m.height as i32 {
                let yy = gy + row;
                if yy < clip.y || yy >= clip.bottom() { continue; }
                let lean = ((base - yy) as f32 * self.slant).round() as i32;
                for col in 0..m.width as i32 {
                    let xx = gx + col + lean;
                    if xx < clip.x || xx >= clip.right() { continue; }
                    let cov = g.bitmap[(row * m.width as i32 + col) as usize] as u32;
                    if cov == 0 { continue; }
                    let i = yy as usize * fw + xx as usize;
                    let d = self.fb.px[i];
                    let (dr, dg, db) = ((d >> 16) & 255, (d >> 8) & 255, d & 255);
                    let inv = 255 - cov;
                    let r = (cr * cov + dr * inv) / 255; let gg = (cg * cov + dg * inv) / 255; let b = (cb * cov + db * inv) / 255;
                    self.fb.px[i] = (r << 16) | (gg << 8) | b;
                }
            }
            pen += m.advance_width;
        }
        ((pen - self.dx(x) as f32) / self.s).round() as i32
    }
    /// Oblique text: every row leans right the further it is above the baseline, which is how the
    /// Preferences module titles get the Helvetica Oblique of the original out of an upright face.
    /// The advance is the upright one, so leave room for the last glyph's lean.
    pub fn text_oblique(&mut self, font: FontId, size: i32, x: i32, y_base: i32, text: &str, color: u32) -> i32 {
        self.slant = 0.21;
        let w = self.text(font, size, x, y_base, text, color);
        self.slant = 0.0;
        w
    }
    /// Baseline (logical) that vertically centers `size` text in a box starting at `y` with height `h`.
    pub fn baseline(&self, font: FontId, size: i32, y: i32, h: i32) -> i32 {
        let lm = self.fonts.line(font, self.px(size));
        let text_h = (lm.ascent - lm.descent) / self.s;
        let asc = lm.ascent / self.s;
        y + ((h as f32 - text_h) / 2.0 + asc).round() as i32
    }
    /// Text in a box, aligned, vertically centered, clipped to the box.
    pub fn text_in(&mut self, font: FontId, size: i32, r: Rect, align: Align, text: &str, color: u32) {
        let w = self.text_width(font, size, text);
        let x = match align { Align::Left => r.x, Align::Center => r.x + (r.w - w) / 2 };
        let base = self.baseline(font, size, r.y, r.h);
        self.push_clip(r);
        self.text(font, size, x, base, text, color);
        self.pop_clip();
    }
    /// Truncate with a trailing ellipsis so the text fits in `max_w`.
    pub fn ellipsize(&self, font: FontId, size: i32, text: &str, max_w: i32) -> String {
        if self.text_width(font, size, text) <= max_w { return text.to_string(); }
        let chars: Vec<char> = text.chars().collect();
        for n in (0..chars.len()).rev() {
            let s: String = chars[..n].iter().collect::<String>() + "…";
            if self.text_width(font, size, &s) <= max_w { return s; }
        }
        "…".into()
    }
    /// Middle-ellipsis truncation for file names (§6.3).
    pub fn ellipsize_mid(&self, font: FontId, size: i32, text: &str, max_w: i32) -> String {
        if self.text_width(font, size, text) <= max_w { return text.to_string(); }
        let chars: Vec<char> = text.chars().collect();
        for keep in (1..chars.len()).rev() {
            let head = (keep as f32 * 0.6).ceil() as usize;
            let tail = keep - head;
            let s: String = chars[..head].iter().collect::<String>() + "…" + &chars[chars.len() - tail..].iter().collect::<String>();
            if self.text_width(font, size, &s) <= max_w { return s; }
        }
        "…".into()
    }
}

/// Encode an 0RGB frame as PNG (headless screenshots).
pub fn encode_png(w: u32, h: u32, px: &[u32]) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        let mut wr = enc.write_header().unwrap();
        let data: Vec<u8> = px.iter().flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, *p as u8]).collect();
        wr.write_image_data(&data).unwrap();
    }
    out
}

/// Encode straight (non-premultiplied) RGBA as PNG.
pub fn encode_png_rgba(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(rgba).unwrap();
    }
    out
}

#[cfg(test)]
mod png_tests {
    use super::*;

    #[test]
    fn decodes_rgb_and_rgba_at_eight_and_sixteen_bits() {
        for (color, depth, samples, expected) in [
            (png::ColorType::Rgb, png::BitDepth::Sixteen, vec![255, 255, 0, 0, 0, 0], [255, 0, 0, 255]),
            (png::ColorType::Rgba, png::BitDepth::Sixteen, vec![255, 255, 0, 0, 0, 0, 128, 128], [128, 0, 0, 128]),
            (png::ColorType::Rgb, png::BitDepth::Eight, vec![255, 0, 0], [255, 0, 0, 255]),
            (png::ColorType::Rgba, png::BitDepth::Eight, vec![255, 0, 0, 128], [128, 0, 0, 128]),
        ] {
            let mut bytes = vec![];
            let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
            encoder.set_color(color);
            encoder.set_depth(depth);
            encoder.write_header().unwrap().write_image_data(&samples).unwrap();
            let image = decode_png(&bytes).unwrap();
            assert_eq!((image.w, image.h), (1, 1));
            assert_eq!(image.px, vec![expected]);
        }
    }
}
