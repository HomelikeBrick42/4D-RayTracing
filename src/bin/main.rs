use eframe::wgpu;
use four_dimentions::App;

fn main() {
    eframe::run_native(
        "4D Ray Tracing",
        eframe::NativeOptions {
            renderer: eframe::Renderer::Wgpu,
            wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
                device_descriptor: wgpu::DeviceDescriptor {
                    ..Default::default()
                },
                present_mode: wgpu::PresentMode::AutoNoVsync,
                power_preference: wgpu::PowerPreference::HighPerformance,
                ..Default::default()
            },
            ..Default::default()
        },
        Box::new(|cc| Box::new(App::new(cc))),
    )
    .unwrap()
}
