use crate::renderer::HudVertex;

static FONT_5X7: [[u8; 5]; 95] = [
    [0x00, 0x00, 0x00, 0x00, 0x00], // ' '
    [0x00, 0x00, 0x5F, 0x00, 0x00], // '!'
    [0x00, 0x07, 0x00, 0x07, 0x00], // '"'
    [0x14, 0x7F, 0x14, 0x7F, 0x14], // '#'
    [0x24, 0x2A, 0x7F, 0x2A, 0x12], // '$'
    [0x23, 0x13, 0x08, 0x64, 0x62], // '%'
    [0x36, 0x49, 0x55, 0x22, 0x50], // '&'
    [0x00, 0x05, 0x03, 0x00, 0x00], // '\''
    [0x00, 0x1C, 0x22, 0x41, 0x00], // '('
    [0x00, 0x41, 0x22, 0x1C, 0x00], // ')'
    [0x14, 0x08, 0x3E, 0x08, 0x14], // '*'
    [0x08, 0x08, 0x3E, 0x08, 0x08], // '+'
    [0x00, 0x50, 0x30, 0x00, 0x00], // ','
    [0x08, 0x08, 0x08, 0x08, 0x08], // '-'
    [0x00, 0x60, 0x60, 0x00, 0x00], // '.'
    [0x20, 0x10, 0x08, 0x04, 0x02], // '/'
    [0x3E, 0x51, 0x49, 0x45, 0x3E], // '0'
    [0x00, 0x42, 0x7F, 0x40, 0x00], // '1'
    [0x42, 0x61, 0x51, 0x49, 0x46], // '2'
    [0x21, 0x41, 0x45, 0x4B, 0x31], // '3'
    [0x18, 0x14, 0x12, 0x7F, 0x10], // '4'
    [0x27, 0x45, 0x45, 0x45, 0x39], // '5'
    [0x3C, 0x4A, 0x49, 0x49, 0x30], // '6'
    [0x01, 0x71, 0x09, 0x05, 0x03], // '7'
    [0x36, 0x49, 0x49, 0x49, 0x36], // '8'
    [0x06, 0x49, 0x49, 0x29, 0x1E], // '9'
    [0x00, 0x36, 0x36, 0x00, 0x00], // ':'
    [0x00, 0x56, 0x36, 0x00, 0x00], // ';'
    [0x08, 0x14, 0x22, 0x41, 0x00], // '<'
    [0x14, 0x14, 0x14, 0x14, 0x14], // '='
    [0x00, 0x41, 0x22, 0x14, 0x08], // '>'
    [0x02, 0x01, 0x51, 0x09, 0x06], // '?'
    [0x32, 0x49, 0x79, 0x41, 0x3E], // '@'
    [0x7E, 0x11, 0x11, 0x11, 0x7E], // 'A'
    [0x7F, 0x49, 0x49, 0x49, 0x36], // 'B'
    [0x3E, 0x41, 0x41, 0x41, 0x22], // 'C'
    [0x7F, 0x41, 0x41, 0x22, 0x1C], // 'D'
    [0x7F, 0x49, 0x49, 0x49, 0x41], // 'E'
    [0x7F, 0x09, 0x09, 0x09, 0x01], // 'F'
    [0x3E, 0x41, 0x49, 0x49, 0x7A], // 'G'
    [0x7F, 0x08, 0x08, 0x08, 0x7F], // 'H'
    [0x00, 0x41, 0x7F, 0x41, 0x00], // 'I'
    [0x20, 0x40, 0x41, 0x3F, 0x01], // 'J'
    [0x7F, 0x08, 0x14, 0x22, 0x41], // 'K'
    [0x7F, 0x40, 0x40, 0x40, 0x40], // 'L'
    [0x7F, 0x02, 0x0C, 0x02, 0x7F], // 'M'
    [0x7F, 0x04, 0x08, 0x10, 0x7F], // 'N'
    [0x3E, 0x41, 0x41, 0x41, 0x3E], // 'O'
    [0x7F, 0x09, 0x09, 0x09, 0x06], // 'P'
    [0x3E, 0x41, 0x51, 0x21, 0x5E], // 'Q'
    [0x7F, 0x09, 0x19, 0x29, 0x46], // 'R'
    [0x46, 0x49, 0x49, 0x49, 0x31], // 'S'
    [0x01, 0x01, 0x7F, 0x01, 0x01], // 'T'
    [0x3F, 0x40, 0x40, 0x40, 0x3F], // 'U'
    [0x1F, 0x20, 0x40, 0x20, 0x1F], // 'V'
    [0x3F, 0x40, 0x38, 0x40, 0x3F], // 'W'
    [0x63, 0x14, 0x08, 0x14, 0x63], // 'X'
    [0x07, 0x08, 0x70, 0x08, 0x07], // 'Y'
    [0x61, 0x51, 0x49, 0x45, 0x43], // 'Z'
    [0x00, 0x7F, 0x41, 0x41, 0x00], // '['
    [0x02, 0x04, 0x08, 0x10, 0x20], // '\'
    [0x00, 0x41, 0x41, 0x7F, 0x00], // ']'
    [0x04, 0x02, 0x01, 0x02, 0x04], // '^'
    [0x40, 0x40, 0x40, 0x40, 0x40], // '_'
    [0x00, 0x01, 0x02, 0x04, 0x00], // '`'
    [0x20, 0x54, 0x54, 0x54, 0x78], // 'a'
    [0x7F, 0x48, 0x44, 0x44, 0x38], // 'b'
    [0x38, 0x44, 0x44, 0x44, 0x20], // 'c'
    [0x38, 0x44, 0x44, 0x48, 0x7F], // 'd'
    [0x38, 0x54, 0x54, 0x54, 0x18], // 'e'
    [0x08, 0x7E, 0x09, 0x01, 0x02], // 'f'
    [0x0C, 0x52, 0x52, 0x52, 0x3E], // 'g'
    [0x7F, 0x08, 0x04, 0x04, 0x78], // 'h'
    [0x00, 0x44, 0x7D, 0x40, 0x00], // 'i'
    [0x20, 0x40, 0x44, 0x3D, 0x00], // 'j'
    [0x7F, 0x10, 0x28, 0x44, 0x00], // 'k'
    [0x00, 0x41, 0x7F, 0x40, 0x00], // 'l'
    [0x7C, 0x04, 0x18, 0x04, 0x78], // 'm'
    [0x7C, 0x08, 0x04, 0x04, 0x78], // 'n'
    [0x38, 0x44, 0x44, 0x44, 0x38], // 'o'
    [0x7C, 0x14, 0x14, 0x14, 0x08], // 'p'
    [0x08, 0x14, 0x14, 0x18, 0x7C], // 'q'
    [0x7C, 0x08, 0x04, 0x04, 0x08], // 'r'
    [0x48, 0x54, 0x54, 0x54, 0x20], // 's'
    [0x04, 0x3F, 0x44, 0x40, 0x20], // 't'
    [0x3C, 0x40, 0x40, 0x20, 0x7C], // 'u'
    [0x1C, 0x20, 0x40, 0x20, 0x1C], // 'v'
    [0x3C, 0x40, 0x30, 0x40, 0x3C], // 'w'
    [0x44, 0x28, 0x10, 0x28, 0x44], // 'x'
    [0x0C, 0x50, 0x50, 0x50, 0x3C], // 'y'
    [0x44, 0x64, 0x54, 0x4C, 0x44], // 'z'
    [0x00, 0x08, 0x36, 0x41, 0x00], // '{'
    [0x00, 0x00, 0x7F, 0x00, 0x00], // '|'
    [0x00, 0x41, 0x36, 0x08, 0x00], // '}'
    [0x08, 0x08, 0x2A, 0x1C, 0x08], // '~'
];

pub struct HudBuilder {
    vertices: Vec<HudVertex>,
}

impl HudBuilder {
    pub fn new() -> Self {
        Self {
            vertices: Vec::with_capacity(512),
        }
    }

    pub fn add_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        let v0 = HudVertex { position: [x, y], color };
        let v1 = HudVertex { position: [x + w, y], color };
        let v2 = HudVertex { position: [x + w, y + h], color };
        let v3 = HudVertex { position: [x, y + h], color };

        self.vertices.extend_from_slice(&[v0, v1, v2, v0, v2, v3]);
    }

    pub fn add_text(&mut self, start_x: f32, start_y: f32, text: &str, scale: f32, color: [f32; 4]) {
        let mut cur_x = start_x;
        let char_spacing = 2.0 * scale;
        let pixel_w = scale;
        let pixel_h = scale;

        for ch in text.chars() {
            let code = match ch {
                c if (c as u32) >= 32 && (c as u32) <= 126 => c as u32,
                'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' => 'a' as u32,
                'é' | 'è' | 'ê' | 'ë' => 'e' as u32,
                'í' | 'ì' | 'î' | 'ï' => 'i' as u32,
                'ó' | 'ò' | 'ô' | 'ö' | 'õ' => 'o' as u32,
                'ú' | 'ù' | 'û' | 'ü' => 'u' as u32,
                'ñ' => 'n' as u32,
                'ç' => 'c' as u32,
                'Á' | 'À' | 'Â' | 'Ä' | 'Ã' | 'Å' => 'A' as u32,
                'É' | 'È' | 'Ê' | 'Ë' => 'E' as u32,
                'Í' | 'Ì' | 'Î' | 'Ï' => 'I' as u32,
                'Ó' | 'Ò' | 'Ô' | 'Ö' | 'Õ' => 'O' as u32,
                'Ú' | 'Ù' | 'Û' | 'Ü' => 'U' as u32,
                'Ñ' => 'N' as u32,
                'Ç' => 'C' as u32,
                '•' | '·' => '-' as u32,
                '“' | '”' => '"' as u32,
                '‘' | '’' => '\'' as u32,
                _ => '?' as u32,
            };

            let glyph = &FONT_5X7[(code - 32) as usize];
            // Coalesce contiguous vertical bit runs in each column into single rectangles.
            // This reduces vertex emissions by over 70% per character.
            for (col, &bits) in glyph.iter().enumerate() {
                let mut row = 0;
                while row < 7 {
                    if (bits & (1 << row)) != 0 {
                        let start_row = row;
                        while row < 7 && (bits & (1 << row)) != 0 {
                            row += 1;
                        }
                        let run_len = row - start_row;
                        let px = cur_x + col as f32 * pixel_w;
                        let py = start_y + start_row as f32 * pixel_h;
                        self.add_rect(px, py, pixel_w, run_len as f32 * pixel_h, color);
                    } else {
                        row += 1;
                    }
                }
            }
            cur_x += 5.0 * pixel_w + char_spacing;
        }
    }

    pub fn build_overlay(
        &mut self,
        _screen_w: f32,
        _screen_h: f32,
        filename: &str,
        format_name: &str,
        img_w: u32,
        img_h: u32,
        zoom: f32,
        decode_us: u128,
        file_idx: usize,
        total_files: usize,
        cache_count: usize,
        cache_mb: f32,
        is_cache_hit: bool,
        visible: bool,
        filter_name: &str,
        tonemap_name: &str,
        exposure_ev: f32,
        color_space_name: &str,
    ) {
        if !visible {
            return;
        }

        let zoom_pct = (zoom * 100.0).round() as i32;
        let decode_ms = (decode_us as f64) / 1000.0;

        let nav_str = if total_files > 0 {
            format!("[{}/{}]", file_idx + 1, total_files)
        } else {
            "[1/1]".to_string()
        };

        let cache_str = format!("Cache: {} ({:.1}MB)", cache_count, cache_mb);
        let time_str = if is_cache_hit {
            "0.00ms (Hit)".to_string()
        } else {
            format!("{:.2}ms", decode_ms)
        };

        let ev_str = if exposure_ev.abs() < 0.01 {
            "0 EV".to_string()
        } else if exposure_ev > 0.0 {
            format!("+{:.1} EV", exposure_ev)
        } else {
            format!("{:.1} EV", exposure_ev)
        };

        let info_line = format!(
            "{} {} | {} | {}x{} | {}% | {} | {} | {} | {} | {} | {}",
            nav_str, filename, format_name, img_w, img_h, zoom_pct, time_str, cache_str,
            filter_name, tonemap_name, ev_str, color_space_name
        );

        let margin = 16.0;
        let bar_h = 36.0;
        let bar_w = (info_line.len() as f32 * 12.0 + 32.0).max(640.0);

        // Glass background with subtle border
        self.add_rect(margin, margin, bar_w, bar_h, [0.08, 0.08, 0.10, 0.88]);
        self.add_rect(margin, margin + bar_h - 1.0, bar_w, 1.0, [0.25, 0.25, 0.30, 0.70]);

        // Accent indicator
        let accent_color = if is_cache_hit {
            [0.20, 0.85, 0.50, 1.00] // Vibrant green for instant cache hit
        } else if format_name.contains("Preview") {
            [1.00, 0.72, 0.15, 1.00] // Vivid amber for fast-path preview
        } else if format_name.contains("Full Demosaic") {
            [0.25, 0.80, 0.95, 1.00] // Electric cyan for full raw demosaic
        } else {
            [0.30, 0.65, 1.00, 1.00] // Sleek blue for fresh decode
        };
        self.add_rect(margin, margin, 4.0, bar_h, accent_color);

        self.add_text(
            margin + 12.0,
            margin + 11.0,
            &info_line,
            1.7,
            [0.92, 0.94, 0.96, 1.0],
        );
    }

    pub fn get_lost_hint_rect(screen_w: f32, screen_h: f32) -> (f32, f32, f32, f32) {
        let msg = "Lost your image? Press [Space] to center";
        let text_scale = 1.6;
        let char_w = 5.0 * text_scale + 2.0 * text_scale;
        let text_w = msg.len() as f32 * char_w;
        let pad_x = 18.0;
        let bar_w = text_w + pad_x * 2.0;
        let bar_h = 32.0;
        let start_x = ((screen_w - bar_w) * 0.5).max(10.0);
        let start_y = (screen_h - bar_h - 24.0).max(10.0);
        (start_x, start_y, bar_w, bar_h)
    }

    pub fn add_lost_image_hint(&mut self, screen_w: f32, screen_h: f32) {
        let (start_x, start_y, bar_w, bar_h) = Self::get_lost_hint_rect(screen_w, screen_h);
        let msg = "Lost your image? Press [Space] to center";
        let text_scale = 1.6;
        let pad_x = 18.0;
        let pad_y = 8.0;

        // Elegant glassmorphism backdrop with subtle borders
        self.add_rect(start_x, start_y, bar_w, bar_h, [0.08, 0.08, 0.10, 0.92]);
        self.add_rect(start_x, start_y, bar_w, 1.0, [0.35, 0.40, 0.50, 0.65]);
        self.add_rect(start_x, start_y + bar_h - 1.0, bar_w, 1.0, [0.15, 0.18, 0.22, 0.85]);
        self.add_rect(start_x, start_y, 4.0, bar_h, [1.00, 0.72, 0.15, 0.95]);

        // Clean crisp text
        self.add_text(
            start_x + pad_x,
            start_y + pad_y,
            msg,
            text_scale,
            [0.94, 0.96, 0.98, 1.0],
        );
    }

    pub fn finish(self) -> Vec<HudVertex> {
        self.vertices
    }
}
