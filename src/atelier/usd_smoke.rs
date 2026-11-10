//! Opt-in native USD launch/capture smoke used by portable packaging.
//! Captures the USD viewport, not the surrounding CPU-painted UI chrome.
use cssimpler::renderer::{self, EngineEvent, gpu::*};
use std::{cell::Cell, path::PathBuf, rc::Rc};

pub struct SmokeViewport {
    pub renderer: usd::UsdRenderer,
    pub output: PathBuf,
    pub done: Rc<Cell<bool>>,
    pub redraw: GpuRedrawHandle,
    pub frames: u32,
}
fn error(e: impl std::fmt::Display) -> renderer::RendererError {
    renderer::RendererError::Surface(e.to_string())
}
impl GpuViewportRenderer for SmokeViewport {
    fn resize(&mut self, gpu: &GpuContext, info: GpuViewportInfo) -> renderer::Result<()> {
        self.renderer.resize(gpu, info)
    }
    fn event(&mut self, event: &EngineEvent) -> bool {
        self.renderer.event(event)
    }
    fn render(&mut self, gpu: &GpuContext, frame: GpuViewportFrame<'_>) -> renderer::Result<()> {
        let info = frame.info;
        let time = frame.frame;
        self.renderer.render(gpu, frame)?;
        self.frames += 1;
        if self.frames < 2 {
            self.redraw.request_redraw();
            return Ok(());
        }
        if self.done.get() {
            return Ok(());
        }
        let [width, height] = info.physical_size;
        if !matches!(
            info.format,
            wgpu::TextureFormat::Bgra8Unorm
                | wgpu::TextureFormat::Bgra8UnormSrgb
                | wgpu::TextureFormat::Rgba8Unorm
                | wgpu::TextureFormat::Rgba8UnormSrgb
        ) {
            return Err(error("Smoke capture requires an RGBA8/BGRA8 surface"));
        }
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("USD smoke capture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: info.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let target = texture.create_view(&Default::default());
        let row = (width * 4).div_ceil(256) * 256;
        let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("USD smoke readback"),
            size: u64::from(row) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        self.renderer.render(
            gpu,
            GpuViewportFrame {
                info,
                frame: time,
                target: &target,
                texture: &texture,
                encoder: &mut encoder,
            },
        )?;
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: None,
                },
            },
            size,
        );
        gpu.queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer.map_async(wgpu::MapMode::Read, .., move |r| {
            let _ = tx.send(r);
        });
        gpu.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(error)?;
        rx.recv().map_err(error)?.map_err(error)?;
        let bytes = buffer.get_mapped_range(..).map_err(error)?;
        let mut pixels: Vec<u8> = bytes
            .chunks_exact(row as usize)
            .flat_map(|r| r[..width as usize * 4].iter().copied())
            .collect();
        drop(bytes);
        buffer.unmap();
        if matches!(
            info.format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        ) {
            for p in pixels.as_chunks_mut::<4>().0 {
                p.swap(0, 2);
            }
        }
        if pixels.as_chunks::<4>().0.iter().all(|p| p == &pixels[..4]) {
            return Err(error(
                "USD smoke capture is uniform; no scene geometry rendered",
            ));
        }
        image::save_buffer(
            &self.output,
            &pixels,
            width,
            height,
            image::ColorType::Rgba8,
        )
        .map_err(error)?;
        std::fs::write(self.output.with_extension("json"), serde_json::json!({"scope":"native USD viewport capture; excludes UI chrome and editing", "native_frames":self.frames,"width":width,"height":height}).to_string()).map_err(error)?;
        self.done.set(true);
        Ok(())
    }
}
