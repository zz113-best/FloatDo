//! AI 人像分割：u2netp（rembg 同款轻量显著性主体分割模型，约 4.5MB）
//! + tract 纯 Rust ONNX 推理，不依赖任何动态库或 Python。
//! 模型首次使用时从 GitHub 下载缓存到应用数据目录，之后完全离线可用。

use std::path::{Path, PathBuf};

/// u2netp 输入尺寸（rembg 发布的 onnx 固定 1×3×320×320）。
pub const INPUT_SIDE: usize = 320;
pub const MODEL_FILE_NAME: &str = "u2netp.onnx";
/// 按顺序尝试的下载源：GitHub 直连 + 镜像（镜像站可用性会变，失败自动换下一个）。
const MODEL_URLS: &[&str] = &[
    "https://github.com/danielgatis/rembg/releases/download/v0.0.0/u2netp.onnx",
    "https://gh-proxy.com/https://github.com/danielgatis/rembg/releases/download/v0.0.0/u2netp.onnx",
];
/// 小于 1MB 视为下载损坏的半截文件。
const MIN_MODEL_BYTES: u64 = 1_000_000;

/// u2net 标准预处理均值/方差（ImageNet）。
const MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const STD: [f32; 3] = [0.229, 0.224, 0.225];

/// 确保模型已就位并返回路径；没下载过（或上次下载损坏）就现场下载。
/// 失败时给出人工放置指引，用户手动下载后重试即可。
pub fn ensure_model(model_dir: &Path) -> Result<PathBuf, String> {
    let path = model_dir.join(MODEL_FILE_NAME);
    if path.is_file()
        && std::fs::metadata(&path)
            .map(|m| m.len() >= MIN_MODEL_BYTES)
            .unwrap_or(false)
    {
        return Ok(path);
    }
    let _ = std::fs::remove_file(&path);
    std::fs::create_dir_all(model_dir).map_err(|e| format!("创建模型目录失败: {e}"))?;
    let tmp = model_dir.join(format!("{MODEL_FILE_NAME}.download"));
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| format!("初始化下载客户端失败: {e}"))?;
    let mut last_err: Option<String> = None;
    for url in MODEL_URLS {
        let bytes = client
            .get(*url)
            .send()
            .and_then(|r| r.error_for_status())
            .and_then(|r| r.bytes());
        match bytes {
            Ok(bytes) => {
                std::fs::write(&tmp, &bytes).map_err(|e| format!("保存模型失败: {e}"))?;
                std::fs::rename(&tmp, &path).map_err(|e| format!("保存模型失败: {e}"))?;
                return Ok(path);
            }
            Err(e) => last_err = Some(format!("{url} → {e}")),
        }
    }
    Err(format!(
        "下载人像分割模型失败（{}）。可手动下载任意一个地址的文件，\
         改名 {} 放到 {} 后重试",
        last_err.unwrap_or_default(),
        MODEL_FILE_NAME,
        path.display()
    ))
}

/// u2net 预处理：缩放到 320×320、RGB 转 NCHW f32 并按 ImageNet 归一化。
pub fn preprocess(img: &image::RgbaImage) -> Vec<f32> {
    let side = INPUT_SIDE as u32;
    let resized = image::imageops::resize(
        img,
        side,
        side,
        image::imageops::FilterType::Triangle,
    );
    let mut out = vec![0f32; 3 * INPUT_SIDE * INPUT_SIDE];
    for j in 0..side {
        for i in 0..side {
            let p = resized.get_pixel(i, j);
            let idx = (j * side + i) as usize;
            for (c, ch) in p.0.iter().take(3).enumerate() {
                out[c * INPUT_SIDE * INPUT_SIDE + idx] =
                    (*ch as f32 / 255.0 - MEAN[c]) / STD[c];
            }
        }
    }
    out
}

/// 跑一次推理，返回 320×320 的软蒙版（0..1，值越大越可能是前景人物）。
pub fn segment_person(model_path: &Path, img: &image::RgbaImage) -> Result<Vec<f32>, String> {
    use tract_onnx::prelude::*;

    let input = preprocess(img);
    // tract 0.23：dt_shape 直接返回 TypedFact（形状不合法会 panic，这里形状是常量）
    let fact = TypedFact::dt_shape(f32::datum_type(), &[1usize, 3, INPUT_SIDE, INPUT_SIDE]);
    let runnable = tract_onnx::onnx()
        .model_for_path(model_path)
        .map_err(|e| format!("加载分割模型失败: {e}"))?
        .with_input_fact(0, fact.into())
        .map_err(|e| format!("设置分割输入规格失败: {e}"))?
        .into_optimized()
        .map_err(|e| format!("优化分割模型失败: {e}"))?
        .into_runnable()
        .map_err(|e| format!("编译分割模型失败: {e}"))?;
    let tensor =
        Tensor::from_shape(&[1usize, 3, INPUT_SIDE, INPUT_SIDE], &input)
            .map_err(|e| format!("构造分割输入张量失败: {e}"))?;
    let outputs = runnable
        .run(tvec!(tensor.into()))
        .map_err(|e| format!("人像分割推理失败: {e}"))?;
    let view = outputs[0]
        .to_plain_array_view::<f32>()
        .map_err(|e| format!("读取分割结果失败: {e}"))?;
    let raw: Vec<f32> = view.iter().copied().collect();
    Ok(normalize_mask(&raw))
}

/// min-max 归一化到 0..1（与 rembg 的后处理一致，保留软边缘）。
pub fn normalize_mask(mask: &[f32]) -> Vec<f32> {
    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    for v in mask {
        min = min.min(*v);
        max = max.max(*v);
    }
    let span = (max - min).max(1e-6);
    mask.iter()
        .map(|v| ((v - min) / span).clamp(0.0, 1.0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "需要网络：下载 u2netp 模型（约 4.5MB）并跑一次真实推理"]
    fn segment_real_model_end_to_end() {
        let tmp = tempfile::tempdir().unwrap();
        let path = ensure_model(tmp.path()).expect("模型应能下载");
        let img = image::RgbaImage::from_pixel(200, 200, image::Rgba([200, 180, 160, 255]));
        let mask = segment_person(&path, &img).expect("推理应成功");
        assert_eq!(mask.len(), INPUT_SIDE * INPUT_SIDE);
        assert!(mask.iter().all(|v| (0.0..=1.0).contains(v)), "蒙版应归一化到 0..1");
    }

    #[test]
    fn preprocess_layout_is_nchw_normalized() {
        // 1×1 纯白图 → 三通道全部是 (1 - mean) / std
        let img = image::RgbaImage::from_pixel(4, 4, image::Rgba([255, 255, 255, 255]));
        let out = preprocess(&img);
        assert_eq!(out.len(), 3 * INPUT_SIDE * INPUT_SIDE);
        for c in 0..3 {
            let expected = (1.0 - MEAN[c]) / STD[c];
            assert!((out[c * INPUT_SIDE * INPUT_SIDE] - expected).abs() < 1e-6);
        }
    }

    #[test]
    fn normalize_mask_maps_min_max_to_unit_range() {
        let mask = normalize_mask(&[2.0, 4.0, 6.0, 3.0]);
        assert!((mask[0] - 0.0).abs() < 1e-6);
        assert!((mask[2] - 1.0).abs() < 1e-6);
        assert!((mask[1] - 0.5).abs() < 1e-6);
        // 常量蒙版不会除零（span 有下限，整体收敛到 0）
        let flat = normalize_mask(&[5.0; 4]);
        assert!(flat.iter().all(|v| v.abs() < 1e-5));
    }
}
