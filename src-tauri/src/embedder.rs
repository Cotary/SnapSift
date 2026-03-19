use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Mutex;

use ndarray::Array4;
use tract_onnx::prelude::*;

type TractModel = SimplePlan<TypedFact, Box<dyn TypedOp>, Graph<TypedFact, Box<dyn TypedOp>>>;

const MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const STD: [f32; 3] = [0.229, 0.224, 0.225];
const MODEL_FILENAME: &str = "mobilenet_v3_small.onnx";

// ---------------------------------------------------------------------------
// Engine selection (runtime-switchable)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AiEngine {
    Tract = 0,
    Ort = 1,
}

static ACTIVE_ENGINE: AtomicU8 = AtomicU8::new(0);

pub fn set_engine(engine: AiEngine) {
    ACTIVE_ENGINE.store(engine as u8, Ordering::Relaxed);
    log::info!("AI engine switched to {:?}", engine);
}

pub fn get_engine() -> AiEngine {
    match ACTIVE_ENGINE.load(Ordering::Relaxed) {
        1 => AiEngine::Ort,
        _ => AiEngine::Tract,
    }
}

pub fn engine_name(engine: AiEngine) -> &'static str {
    match engine {
        AiEngine::Tract => "tract (Pure Rust)",
        AiEngine::Ort => "ort (ONNX Runtime)",
    }
}

// ---------------------------------------------------------------------------
// Model path (shared between both engines)
// ---------------------------------------------------------------------------

static MODEL_PATH: std::sync::LazyLock<Mutex<Option<PathBuf>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));

fn find_model(resource_dir: &Path) -> Option<PathBuf> {
    let candidates = [
        resource_dir.join(MODEL_FILENAME),
        resource_dir.join("resources").join(MODEL_FILENAME),
        PathBuf::from("resources").join(MODEL_FILENAME),
    ];
    candidates.into_iter().find(|p| p.exists())
}

pub fn init_embedder(resource_dir: &Path) {
    let model_path = match find_model(resource_dir) {
        Some(p) => p,
        None => {
            log::warn!(
                "AI model not found. Place {} in src-tauri/resources/",
                MODEL_FILENAME
            );
            return;
        }
    };

    // Quick verify with tract (lightweight pure-rust check)
    if load_tract_model(&model_path).is_some() {
        log::info!("AI model verified at {:?}", model_path);
        *MODEL_PATH.lock().unwrap() = Some(model_path);
    } else {
        log::warn!("AI model at {:?} failed to load.", model_path);
    }
}

pub fn is_available() -> bool {
    MODEL_PATH.lock().unwrap().is_some()
}

// ---------------------------------------------------------------------------
// Shared preprocessing
// ---------------------------------------------------------------------------

fn preprocess_image(image_path: &str) -> Option<Array4<f32>> {
    let img = image::open(image_path).ok()?;
    let resized = img.resize_exact(224, 224, image::imageops::FilterType::Triangle);
    let rgb = resized.to_rgb8();

    let mut input = Array4::<f32>::zeros((1, 3, 224, 224));
    for y in 0..224u32 {
        for x in 0..224u32 {
            let pixel = rgb.get_pixel(x, y);
            for c in 0..3 {
                let val = pixel[c] as f32 / 255.0;
                input[[0, c, y as usize, x as usize]] = (val - MEAN[c]) / STD[c];
            }
        }
    }
    Some(input)
}

fn normalize_l2(vec: Vec<f32>) -> Option<Vec<f32>> {
    if vec.is_empty() {
        return None;
    }
    let norm: f32 = vec.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm < 1e-8 {
        return Some(vec);
    }
    Some(vec.iter().map(|v| v / norm).collect())
}

// ---------------------------------------------------------------------------
// Tract backend
// ---------------------------------------------------------------------------

thread_local! {
    static LOCAL_TRACT: RefCell<Option<TractModel>> = const { RefCell::new(None) };
}

fn load_tract_model(path: &Path) -> Option<TractModel> {
    tract_onnx::onnx()
        .model_for_path(path)
        .and_then(|m| m.into_optimized())
        .and_then(|m| m.into_runnable())
        .ok()
}

fn with_tract<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&TractModel) -> Option<R>,
{
    LOCAL_TRACT.with(|cell| {
        let mut model_ref = cell.borrow_mut();
        if model_ref.is_none() {
            let path_guard = MODEL_PATH.lock().unwrap();
            if let Some(ref p) = *path_guard {
                *model_ref = load_tract_model(p);
            }
        }
        model_ref.as_ref().and_then(f)
    })
}

fn extract_feature_tract(image_path: &str) -> Option<Vec<f32>> {
    let arr = preprocess_image(image_path)?;
    let tensor: Tensor = arr.into();
    with_tract(|model| {
        let result = model.run(tvec![tensor.clone().into()]).ok()?;
        let output = result[0].to_array_view::<f32>().ok()?;
        let vec: Vec<f32> = output.iter().copied().collect();
        normalize_l2(vec)
    })
}

// ---------------------------------------------------------------------------
// Ort backend
// ---------------------------------------------------------------------------

thread_local! {
    static LOCAL_ORT: RefCell<Option<ort::session::Session>> = const { RefCell::new(None) };
}

fn load_ort_session(path: &Path) -> Option<ort::session::Session> {
    ort::session::Session::builder()
        .ok()?
        .with_intra_threads(1)
        .ok()?
        .commit_from_file(path)
        .ok()
}

fn with_ort<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut ort::session::Session) -> Option<R>,
{
    LOCAL_ORT.with(|cell| {
        let mut session_ref = cell.borrow_mut();
        if session_ref.is_none() {
            let path_guard = MODEL_PATH.lock().unwrap();
            if let Some(ref p) = *path_guard {
                *session_ref = load_ort_session(p);
            }
        }
        session_ref.as_mut().and_then(f)
    })
}

fn extract_feature_ort(image_path: &str) -> Option<Vec<f32>> {
    let arr = preprocess_image(image_path)?;
    let raw: Vec<f32> = arr.iter().copied().collect();
    with_ort(|session| {
        let shape: [usize; 4] = [1, 3, 224, 224];
        let input_ref = ort::value::TensorRef::from_array_view((shape, raw.as_slice())).ok()?;
        let outputs = session.run(ort::inputs![input_ref]).ok()?;
        let (_, output_data) = outputs[0].try_extract_tensor::<f32>().ok()?;
        let vec: Vec<f32> = output_data.to_vec();
        normalize_l2(vec)
    })
}

// ---------------------------------------------------------------------------
// Public API (dispatches to active engine)
// ---------------------------------------------------------------------------

pub fn extract_feature(image_path: &str) -> Option<Vec<f32>> {
    match get_engine() {
        AiEngine::Tract => extract_feature_tract(image_path),
        AiEngine::Ort => extract_feature_ort(image_path),
    }
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

pub fn feature_to_bytes(vec: &[f32]) -> Vec<u8> {
    vec.iter().flat_map(|f| f.to_le_bytes()).collect()
}

pub fn bytes_to_feature(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}
