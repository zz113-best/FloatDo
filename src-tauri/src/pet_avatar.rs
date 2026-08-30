//! 桌宠头像处理：从照片里抠出人物主体（剔除背景），再套用视觉风格。
//! 全部是纯本地图像算法（区域生长 + 形态学/重采样），不依赖网络与模型文件；
//! 与命令层分离成纯函数，便于单元测试。

use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};
use std::path::Path;

/// 处理前先把长边缩到该尺寸（桌面桌宠显示很小，512 足够且处理快）。
pub const MAX_SIDE: u32 = 512;
/// 最终输出的桌宠形象长边。
pub const OUTPUT_SIDE: u32 = 256;

/// 桌宠视觉风格。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PetStyle {
    /// 原图（只抠掉背景）
    Original,
    /// Q版贴纸：中心放大 + 白边贴纸感
    Chibi,
    /// 二次元：赛璐璐平涂色块
    Anime,
    /// 像素风：像素化 + 减色
    Pixel,
    /// 手绘风：铅笔线稿 + 纸面质感
    Sketch,
}

impl PetStyle {
    pub fn as_str(&self) -> &'static str {
        match self {
            PetStyle::Original => "original",
            PetStyle::Chibi => "chibi",
            PetStyle::Anime => "anime",
            PetStyle::Pixel => "pixel",
            PetStyle::Sketch => "sketch",
        }
    }

    pub fn from_db(value: &str) -> Result<Self, String> {
        match value {
            "original" => Ok(PetStyle::Original),
            "chibi" => Ok(PetStyle::Chibi),
            "anime" => Ok(PetStyle::Anime),
            "pixel" => Ok(PetStyle::Pixel),
            "sketch" => Ok(PetStyle::Sketch),
            other => Err(format!("未知的桌宠风格: {other}")),
        }
    }
}

/// 完整流水线：缩放 → 抠背景（AI 分割或几何算法）→ 套风格 → 输出固定尺寸。
/// `use_ai` 时需要调用方先用 pet_segment::ensure_model 准备好模型并传入路径。
pub fn process_avatar(
    src: &DynamicImage,
    style: PetStyle,
    tolerance: u8,
    use_ai: bool,
    model_path: Option<&Path>,
) -> Result<RgbaImage, String> {
    let rgba = src.to_rgba8();
    let (w, h) = rgba.dimensions();
    let scaled = if w.max(h) > MAX_SIDE {
        let scale = MAX_SIDE as f32 / w.max(h) as f32;
        let nw = ((w as f32 * scale).round() as u32).max(1);
        let nh = ((h as f32 * scale).round() as u32).max(1);
        image::imageops::resize(&rgba, nw, nh, FilterType::Triangle)
    } else {
        rgba
    };
    let cut = if use_ai {
        let model_path =
            model_path.ok_or_else(|| "未提供分割模型路径".to_string())?;
        let mask = crate::pet_segment::segment_person(model_path, &scaled)?;
        apply_mask(&scaled, &mask, crate::pet_segment::INPUT_SIDE as u32)
    } else {
        cutout_background(&scaled, tolerance as i32)?
    };
    let styled = apply_style(&cut, style);
    // 裁掉四周透明边再放大到标准尺寸：桌宠占桌面的就是人物本身的大小
    Ok(fit_output(&crop_to_content(&styled)))
}

/// 把分割软蒙版（side×side 的 0..1 值）缩放回原图尺寸并乘进 alpha。
/// 与原 alpha 取乘积：已透明的区域保持透明。
fn apply_mask(img: &RgbaImage, mask: &[f32], side: u32) -> RgbaImage {
    let (w, h) = img.dimensions();
    let mut mg = image::GrayImage::new(side, side);
    for j in 0..side {
        for i in 0..side {
            let v = mask[(j * side + i) as usize];
            mg.put_pixel(i, j, image::Luma([(v * 255.0).round() as u8]));
        }
    }
    let resized = image::imageops::resize(&mg, w, h, FilterType::Triangle);
    let mut out = img.clone();
    for (x, y, p) in out.enumerate_pixels_mut() {
        let m = resized.get_pixel(x, y).0[0] as u32;
        p.0[3] = ((p.0[3] as u32 * m) / 255) as u8;
    }
    out
}

/// 抠背景：从照片四边做区域生长（相邻像素色差小于容差视为背景），
/// 再只保留最大的一块前景连通域，最后对边缘做 1px 羽化。
pub fn cutout_background(img: &RgbaImage, tolerance: i32) -> Result<RgbaImage, String> {
    let (w, h) = img.dimensions();
    let n = (w as usize) * (h as usize);
    let raw = img.as_raw();

    let rgb_at = |i: u32, j: u32| -> (i32, i32, i32) {
        let idx = ((j * w + i) * 4) as usize;
        (raw[idx] as i32, raw[idx + 1] as i32, raw[idx + 2] as i32)
    };

    // 1. 区域生长：边框全是种子
    let mut bg = vec![false; n];
    let mut queue = std::collections::VecDeque::new();
    // 透明像素（比如已经抠过图的 PNG）直接视为背景
    for j in 0..h {
        for i in 0..w {
            let k = (j * w + i) as usize;
            if !bg[k] && raw[k * 4 + 3] < 32 {
                bg[k] = true;
                queue.push_back((i, j));
            }
        }
    }
    let seed = |i: u32, j: u32, bg: &mut Vec<bool>, queue: &mut std::collections::VecDeque<(u32, u32)>| {
        let k = (j * w + i) as usize;
        if !bg[k] {
            bg[k] = true;
            queue.push_back((i, j));
        }
    };
    for i in 0..w {
        seed(i, 0, &mut bg, &mut queue);
        seed(i, h - 1, &mut bg, &mut queue);
    }
    for j in 0..h {
        seed(0, j, &mut bg, &mut queue);
        seed(w - 1, j, &mut bg, &mut queue);
    }
    while let Some((i, j)) = queue.pop_front() {
        let c = rgb_at(i, j);
        for (di, dj) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
            let ni = i as i64 + di as i64;
            let nj = j as i64 + dj as i64;
            if ni < 0 || nj < 0 || ni >= w as i64 || nj >= h as i64 {
                continue;
            }
            let (ni, nj) = (ni as u32, nj as u32);
            let k = (nj * w + ni) as usize;
            if bg[k] {
                continue;
            }
            let d = rgb_at(ni, nj);
            let diff = [(c.0 - d.0).abs(), (c.1 - d.1).abs(), (c.2 - d.2).abs()]
                .into_iter()
                .max()
                .unwrap_or(0);
            if diff <= tolerance {
                bg[k] = true;
                queue.push_back((ni, nj));
            }
        }
    }

    // 2. 只保留最大的一块前景（去掉背景噪声残留的零碎小岛）
    let mut label = vec![0u32; n];
    let mut sizes: Vec<usize> = vec![0]; // 下标 0 留给背景
    let mut next = 1u32;
    for start in 0..n {
        if bg[start] || label[start] != 0 {
            continue;
        }
        let id = next;
        next += 1;
        let mut count = 0usize;
        let mut stack = vec![start];
        label[start] = id;
        while let Some(k) = stack.pop() {
            count += 1;
            let x = (k as u32) % w;
            let y = (k as u32) / w;
            for (di, dj) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)] {
                let nx = x as i64 + di;
                let ny = y as i64 + dj;
                if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                    continue;
                }
                let nk = ((ny as u32) * w + nx as u32) as usize;
                if !bg[nk] && label[nk] == 0 {
                    label[nk] = id;
                    stack.push(nk);
                }
            }
        }
        sizes.push(count);
    }
    let (best_id, best_size) = sizes
        .iter()
        .enumerate()
        .skip(1)
        .max_by_key(|(_, s)| **s)
        .map(|(id, s)| (id as u32, *s))
        .unwrap_or((0, 0));
    if best_size * 50 < n {
        return Err(
            "没能在照片里找到明确的主体，试试调低「容差」重新抠图（背景越干净效果越好）"
                .to_string(),
        );
    }
    for k in 0..n {
        if !bg[k] && label[k] != best_id {
            bg[k] = true;
        }
    }

    // 3. 边缘羽化：二值蒙版轻微模糊，锯齿过渡成半透明
    let mut mask = image::GrayImage::new(w, h);
    for j in 0..h {
        for i in 0..w {
            let v = if bg[(j * w + i) as usize] { 0u8 } else { 255u8 };
            mask.put_pixel(i, j, image::Luma([v]));
        }
    }
    let soft = image::imageops::blur(&mask, 1.0f32);

    let mut out = RgbaImage::new(w, h);
    for j in 0..h {
        for i in 0..w {
            let mut px = *img.get_pixel(i, j);
            px.0[3] = soft.get_pixel(i, j).0[0];
            if px.0[3] == 0 {
                continue;
            }
            out.put_pixel(i, j, px);
        }
    }
    Ok(out)
}

/// 把风格处理应用到已抠好图（带透明背景）的形象上。
pub fn apply_style(img: &RgbaImage, style: PetStyle) -> RgbaImage {
    match style {
        PetStyle::Original => img.clone(),
        PetStyle::Pixel => saturate_img(&posterize_img(&pixelate(img, 48), 5), 1.1),
        PetStyle::Anime => saturate_img(&posterize_img(img, 7), 1.35),
        PetStyle::Chibi => sticker_outline(&saturate_img(&bulge(img, 0.35), 1.2), 2),
        PetStyle::Sketch => sketch(img),
    }
}

/// 按 alpha 边界裁掉四周的透明区，只保留人物主体（留 2px 呼吸边）。
pub fn crop_to_content(img: &RgbaImage) -> RgbaImage {
    let (w, h) = img.dimensions();
    let mut min_x = w;
    let mut min_y = h;
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    for (x, y, p) in img.enumerate_pixels() {
        if p.0[3] > 16 {
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
        }
    }
    if min_x > max_x || min_y > max_y {
        return img.clone(); // 全透明：原样返回，由上层报错兜底
    }
    let pad = 2u32;
    let x0 = min_x.saturating_sub(pad);
    let y0 = min_y.saturating_sub(pad);
    let x1 = (max_x + 1 + pad).min(w);
    let y1 = (max_y + 1 + pad).min(h);
    img.view(x0, y0, x1 - x0, y1 - y0).to_image()
}

/// 输出统一缩放：长边缩放（含放大）到 OUTPUT_SIDE，保持比例。
fn fit_output(img: &RgbaImage) -> RgbaImage {
    let (w, h) = img.dimensions();
    let long = w.max(h);
    if long == 0 || long == OUTPUT_SIDE {
        return img.clone();
    }
    let s = OUTPUT_SIDE as f32 / long as f32;
    let nw = ((w as f32 * s).round() as u32).max(1);
    let nh = ((h as f32 * s).round() as u32).max(1);
    image::imageops::resize(img, nw, nh, FilterType::Triangle)
}

/// 像素化：缩到 blocks 个像素宽再按最近邻放大回原尺寸。
fn pixelate(img: &RgbaImage, blocks: u32) -> RgbaImage {
    let (w, h) = img.dimensions();
    let small_w = blocks.min(w).max(1);
    let small_h = (((blocks as f32) * h as f32 / w as f32).round() as u32)
        .max(1)
        .min(h.max(1));
    let small = image::imageops::resize(img, small_w, small_h, FilterType::Triangle);
    image::imageops::resize(&small, w, h, FilterType::Nearest)
}

/// 减色：每个通道量化到 levels 级，制造平涂色块感。
fn posterize_img(img: &RgbaImage, levels: u8) -> RgbaImage {
    let mut out = img.clone();
    for p in out.pixels_mut() {
        for ch in p.0.iter_mut().take(3) {
            *ch = posterize_channel(*ch, levels);
        }
    }
    out
}

fn posterize_channel(v: u8, levels: u8) -> u8 {
    if levels <= 1 {
        return v;
    }
    let l = levels as u32;
    let q = (v as u32 * (l - 1) + 127) / 255;
    (q * 255 / (l - 1)) as u8
}

/// 饱和度调整：factor > 1 提升鲜艳度。
fn saturate_img(img: &RgbaImage, factor: f32) -> RgbaImage {
    let mut out = img.clone();
    for p in out.pixels_mut() {
        let (r, g, b) = (p.0[0] as f32, p.0[1] as f32, p.0[2] as f32);
        let gray = 0.299 * r + 0.587 * g + 0.114 * b;
        let f = |c: f32| (gray + (c - gray) * factor).clamp(0.0, 255.0) as u8;
        p.0[0] = f(r);
        p.0[1] = f(g);
        p.0[2] = f(b);
    }
    out
}

/// 中心放大（Q版效果）：逆向映射，越靠近中心采样半径被压缩得越少。
fn bulge(img: &RgbaImage, strength: f32) -> RgbaImage {
    let (w, h) = img.dimensions();
    let mut out = RgbaImage::new(w, h);
    let cx = (w - 1) as f32 / 2.0;
    let cy = (h - 1) as f32 / 2.0;
    let max_r = cx.max(cy).max(1.0);
    for y in 0..h {
        for x in 0..w {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let r = (dx * dx + dy * dy).sqrt();
            if r < 1e-6 {
                out.put_pixel(x, y, *img.get_pixel(cx as u32, cy as u32));
                continue;
            }
            let src_r = max_r * (r / max_r).powf(1.0 + strength);
            let sx = (cx + dx / r * src_r).round() as i64;
            let sy = (cy + dy / r * src_r).round() as i64;
            if sx < 0 || sy < 0 || sx >= w as i64 || sy >= h as i64 {
                continue; // 保持透明
            }
            out.put_pixel(x, y, *img.get_pixel(sx as u32, sy as u32));
        }
    }
    out
}

/// 白边贴纸：把 alpha 蒙版向外扩 radius 像素，先铺白色再叠加原图。
fn sticker_outline(img: &RgbaImage, radius: i32) -> RgbaImage {
    let (w, h) = img.dimensions();
    let mut out = RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let mut max_a = 0u8;
            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    let nx = x as i64 + dx as i64;
                    let ny = y as i64 + dy as i64;
                    if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                        continue;
                    }
                    let a = img.get_pixel(nx as u32, ny as u32).0[3];
                    if a > max_a {
                        max_a = a;
                    }
                }
            }
            // 白底（扩大的 alpha）+ 原图 alpha 混合
            let src = img.get_pixel(x, y);
            let sa = src.0[3] as f32 / 255.0;
            let ba = max_a as f32 / 255.0 * (1.0 - sa);
            let oa = sa + ba;
            if oa <= 0.0 {
                continue;
            }
            let mix = |sc: u8, bc: u8| -> u8 {
                ((sc as f32 * sa + bc as f32 * ba) / oa).round().clamp(0.0, 255.0) as u8
            };
            let a = (oa * 255.0).round().clamp(0.0, 255.0) as u8;
            out.put_pixel(
                x,
                y,
                Rgba([mix(src.0[0], 255), mix(src.0[1], 255), mix(src.0[2], 255), a]),
            );
        }
    }
    out
}

/// 手绘风：亮度图上做 Sobel 边缘检测，边缘画深色线条，其余铺纸面色。
fn sketch(img: &RgbaImage) -> RgbaImage {
    let (w, h) = img.dimensions();
    let luma = |x: u32, y: u32| -> f32 {
        let p = img.get_pixel(x, y);
        if p.0[3] < 32 {
            return 255.0; // 透明处按纸面处理，避免边缘噪声
        }
        0.299 * p.0[0] as f32 + 0.587 * p.0[1] as f32 + 0.114 * p.0[2] as f32
    };
    let mut out = RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let src = img.get_pixel(x, y);
            if src.0[3] < 32 {
                continue;
            }
            let at = |xx: i64, yy: i64| -> f32 {
                let cx = xx.clamp(0, w as i64 - 1) as u32;
                let cy = yy.clamp(0, h as i64 - 1) as u32;
                luma(cx, cy)
            };
            let gx = -at(x as i64 - 1, y as i64 - 1) - 2.0 * at(x as i64 - 1, y as i64)
                - at(x as i64 - 1, y as i64 + 1)
                + at(x as i64 + 1, y as i64 - 1)
                + 2.0 * at(x as i64 + 1, y as i64)
                + at(x as i64 + 1, y as i64 + 1);
            let gy = -at(x as i64 - 1, y as i64 - 1) - 2.0 * at(x as i64, y as i64 - 1)
                - at(x as i64 + 1, y as i64 - 1)
                + at(x as i64 - 1, y as i64 + 1)
                + 2.0 * at(x as i64, y as i64 + 1)
                + at(x as i64 + 1, y as i64 + 1);
            let mag = (gx * gx + gy * gy).sqrt();
            if mag > 70.0 {
                out.put_pixel(x, y, Rgba([74, 69, 64, 255]));
            } else {
                // 纸面：按亮度做 4 级灰阶过渡
                let l = luma(x, y);
                let q = (l / 255.0 * 3.0).round() / 3.0;
                let c = (250.0 - 30.0 * q).round() as u8;
                out.put_pixel(x, y, Rgba([c, (c as f32 * 0.99) as u8, (c as f32 * 0.96) as u8, 255]));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 造一张 24×24 的「照片」：接近纯色的背景 + 中央 8×8 主体。
    fn fixture_photo() -> RgbaImage {
        let mut img = RgbaImage::new(24, 24);
        for j in 0..24 {
            for i in 0..24 {
                // 背景带轻微噪声（差值 ≤ 6，容差 20 内算同一片）
                let n = ((i + j) % 3) as u8;
                img.put_pixel(i, j, Rgba([240 + n, 240, 238 + n, 255]));
            }
        }
        for j in 8..16 {
            for i in 8..16 {
                img.put_pixel(i, j, Rgba([30, 120, 220, 255]));
            }
        }
        img
    }

    #[test]
    fn cutout_keeps_subject_and_drops_flat_background() {
        let cut = cutout_background(&fixture_photo(), 20).expect("抠图应成功");
        // 四角是背景 → 透明
        assert_eq!(cut.get_pixel(0, 0).0[3], 0);
        assert_eq!(cut.get_pixel(23, 23).0[3], 0);
        // 主体中心完全不透明
        assert_eq!(cut.get_pixel(12, 12).0[3], 255);
        // 主体颜色保留
        assert_eq!(cut.get_pixel(12, 12).0, [30, 120, 220, 255]);
    }

    #[test]
    fn cutout_rejects_when_no_subject_found() {
        // 整张图都是接近纯色的「天空」→ 没有主体
        let mut img = RgbaImage::new(24, 24);
        for j in 0..24 {
            for i in 0..24 {
                img.put_pixel(i, j, Rgba([230, 235, 240, 255]));
            }
        }
        let err = cutout_background(&img, 20).unwrap_err();
        assert!(err.contains("主体"));
    }

    #[test]
    fn cutout_treats_transparent_pixels_as_background() {
        // 已抠好、透明底的图：主体直接保留，透明区清空，不误报「找不到主体」
        let mut img = RgbaImage::new(24, 24);
        for j in 8..16 {
            for i in 8..16 {
                img.put_pixel(i, j, Rgba([30, 120, 220, 255]));
            }
        }
        let cut = cutout_background(&img, 20).expect("透明底的图应处理成功");
        assert_eq!(cut.get_pixel(0, 0).0[3], 0);
        assert_eq!(cut.get_pixel(12, 12).0, [30, 120, 220, 255]);
    }

    #[test]
    fn style_roundtrip_through_db_strings() {
        for s in [
            PetStyle::Original,
            PetStyle::Chibi,
            PetStyle::Anime,
            PetStyle::Pixel,
            PetStyle::Sketch,
        ] {
            assert_eq!(PetStyle::from_db(s.as_str()).unwrap(), s);
        }
        assert!(PetStyle::from_db("3d").is_err());
    }

    #[test]
    fn pixel_style_makes_block_uniform_image() {
        // 渐变主体像素化后，同一块内像素应当一致
        let mut img = RgbaImage::new(32, 32);
        for j in 0..32 {
            for i in 0..32 {
                img.put_pixel(i, j, Rgba([(i * 8) as u8, (j * 8) as u8, 128, 255]));
            }
        }
        let out = apply_style(&img, PetStyle::Pixel);
        let distinct_before: std::collections::HashSet<_> = (0..32)
            .flat_map(|i| (0..32).map(move |j| (i, j)))
            .map(|(i, j)| img.get_pixel(i, j).0)
            .collect();
        let distinct_after: std::collections::HashSet<_> = (0..32)
            .flat_map(|i| (0..32).map(move |j| (i, j)))
            .map(|(i, j)| out.get_pixel(i, j).0)
            .collect();
        assert!(
            distinct_after.len() < distinct_before.len(),
            "像素风必须减色: before={} after={}",
            distinct_before.len(),
            distinct_after.len()
        );
    }

    #[test]
    fn chibi_style_adds_white_outline_outside_subject() {
        // 中央 8×8 主体，抠完背景后贴纸白边应出现在主体外 1~2px
        let cut = cutout_background(&fixture_photo(), 20).unwrap();
        let out = apply_style(&cut, PetStyle::Chibi);
        let p = out.get_pixel(6, 12).0; // 主体左边界外 2px（原为背景 → 透明）
        assert!(p[3] > 200, "贴纸白边应不透明: {:?}", p);
        assert!(p[0] > 230 && p[1] > 230 && p[2] > 230, "应是白色: {:?}", p);
    }

    #[test]
    fn sketch_style_draws_dark_lines_on_paper() {
        let cut = cutout_background(&fixture_photo(), 20).unwrap();
        let out = apply_style(&cut, PetStyle::Sketch);
        // 主体边界（x=8 竖线）应是深色线条
        let edge = out.get_pixel(8, 12).0;
        assert!(edge[3] == 255 && edge[0] < 120, "轮廓应是深色线: {:?}", edge);
        // 主体内部平坦区应是纸面色
        let inside = out.get_pixel(12, 12).0;
        assert!(inside[0] > 180, "内部应是纸面浅色: {:?}", inside);
    }

    #[test]
    fn apply_mask_multiplies_alpha() {
        // 4×4 蒙版：左半列全前景、右半列全背景 → 8×8 全不透明图右半变透明
        let img = RgbaImage::from_pixel(8, 8, Rgba([10, 20, 30, 255]));
        let mut mask = Vec::new();
        for _ in 0..4 {
            mask.extend([1.0f32, 1.0, 0.0, 0.0]);
        }
        let out = apply_mask(&img, &mask, 4);
        assert_eq!(out.get_pixel(1, 4).0[3], 255);
        assert_eq!(out.get_pixel(6, 4).0[3], 0);
        // 颜色不受影响
        assert_eq!(out.get_pixel(1, 4).0, [10, 20, 30, 255]);
    }

    #[test]
    fn crop_to_content_trims_transparent_margins() {
        // 32×32 全透明画布 + 中央 8×8 主体 → 裁完只剩主体加 2px 边距
        let mut img = RgbaImage::new(32, 32);
        for j in 12..20 {
            for i in 12..20 {
                img.put_pixel(i, j, Rgba([10, 20, 30, 255]));
            }
        }
        let out = crop_to_content(&img);
        assert_eq!(out.dimensions(), (12, 12)); // 8 + 2×2 边距
        // 内容原样保留
        assert_eq!(out.get_pixel(4, 4).0, [10, 20, 30, 255]);
        assert_eq!(out.get_pixel(0, 0).0[3], 0);
    }

    #[test]
    fn process_avatar_end_to_end_scales_output() {
        // 512+ 的大图进来，输出长边收敛到 OUTPUT_SIDE
        let big = DynamicImage::ImageRgba8(RgbaImage::from_pixel(600, 300, Rgba([200, 200, 200, 255])));
        let mut img = big.to_rgba8();
        for j in 100..200 {
            for i in 250..350 {
                img.put_pixel(i, j, Rgba([200, 40, 40, 255]));
            }
        }
        let big = DynamicImage::ImageRgba8(img);
        let out = process_avatar(&big, PetStyle::Original, 20, false, None).expect("处理应成功");
        assert_eq!(out.dimensions().0.max(out.dimensions().1), OUTPUT_SIDE);
        assert!(out.get_pixel(0, 0).0[3] == 0, "背景应被剔除");
    }
}
