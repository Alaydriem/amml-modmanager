use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use egui::{ColorImage, Context};

// Waits a few frames for each shot, asks the integration for the rendered pixels and saves them as PNG
pub struct ShotRunner {
    out_dir: PathBuf,
    frames: u32,
    requested: bool,
    result: Arc<Mutex<Option<Arc<ColorImage>>>>,
}

impl ShotRunner {
    const WAIT_FRAMES: u32 = 10;

    pub fn new(out_dir: PathBuf) -> Self {
        Self { out_dir, frames: 0, requested: false, result: Arc::new(Mutex::new(None)) }
    }

    // true once the current shot is saved, so the caller moves to the next one
    pub fn tick(&mut self, ctx: &Context, file_name: &str) -> bool {
        self.frames += 1;
        ctx.request_repaint();
        if !self.requested && self.frames >= Self::WAIT_FRAMES {
            self.requested = true;
            let slot = Arc::clone(&self.result);
            ctx.request_screenshot(move |image| {
                if let Ok(mut guard) = slot.lock() {
                    *guard = Some(image);
                }
            });
            return false;
        }
        let image = self.result.lock().ok().and_then(|mut guard| guard.take());
        let Some(image) = image else {
            return false;
        };
        self.save(&image, file_name);
        self.frames = 0;
        self.requested = false;
        true
    }

    fn save(&self, image: &ColorImage, file_name: &str) {
        let [w, h] = image.size;
        let mut bytes = Vec::with_capacity(w * h * 4);
        for pixel in &image.pixels {
            bytes.extend_from_slice(&pixel.to_srgba_unmultiplied());
        }
        if let Err(error) = std::fs::create_dir_all(&self.out_dir) {
            eprintln!("could not create {}: {error}", self.out_dir.display());
            return;
        }
        let path = self.out_dir.join(file_name);
        match image::RgbaImage::from_raw(w as u32, h as u32, bytes) {
            Some(buffer) => match buffer.save(&path) {
                Ok(()) => println!("saved {} ({w}x{h})", path.display()),
                Err(error) => eprintln!("could not save {}: {error}", path.display()),
            },
            None => eprintln!("screenshot buffer size mismatch"),
        }
    }
}
