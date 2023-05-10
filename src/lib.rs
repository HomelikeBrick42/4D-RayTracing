use eframe::{
    egui,
    wgpu::{self, include_wgsl, util::DeviceExt},
};
use encase::{ArrayLength, DynamicStorageBuffer, ShaderSize, ShaderType, UniformBuffer};

mod bivector;
mod rotor;

pub use bivector::*;
pub use rotor::*;

#[derive(Clone, Copy)]
struct Camera {
    pub position: cgmath::Vector4<f32>,
    pub pitch: f32,
    pub yaw: f32,
    pub weird_pitch: f32,
    pub weird_yaw: f32,
    pub fov: f32,
    pub min_distance: f32,
    pub max_distance: f32,
}

#[derive(Clone, Copy, ShaderType)]
struct GpuCamera {
    pub position: cgmath::Vector4<f32>,
    pub forward: cgmath::Vector4<f32>,
    pub right: cgmath::Vector4<f32>,
    pub up: cgmath::Vector4<f32>,
    pub fov: f32,
    pub min_distance: f32,
    pub max_distance: f32,
}

#[derive(Clone, Copy, ShaderType)]
struct GpuHyperSphere {
    pub center: cgmath::Vector4<f32>,
    pub radius: f32,
}

#[derive(Clone, Copy, ShaderType)]
struct GpuHyperSpheres<'a> {
    pub count: ArrayLength,
    #[size(runtime)]
    pub data: &'a [GpuHyperSphere],
}

pub struct App {
    previous_time: std::time::Instant,
    texture_width: usize,
    texture_height: usize,
    texture_id: egui::TextureId,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    texture_bind_group: wgpu::BindGroup,
    camera: Camera,
    camera_uniform_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    hyper_spheres: Vec<GpuHyperSphere>,
    hyper_sphere_names: Vec<String>,
    hyper_spheres_storage_buffer: wgpu::Buffer,
    hyper_spheres_storage_buffer_size: usize,
    objects_bind_group_layout: wgpu::BindGroupLayout,
    objects_bind_group: wgpu::BindGroup,
    ray_tracing_pipeline: wgpu::ComputePipeline,
}

impl App {
    pub fn new(cc: &eframe::CreationContext) -> Self {
        let eframe::egui_wgpu::RenderState {
            device, renderer, ..
        } = cc.wgpu_render_state.as_ref().unwrap();

        let ray_tracing_shader = device.create_shader_module(include_wgsl!("./ray_tracing.wgsl"));

        let texture_width = 1;
        let texture_height = 1;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Texture"),
            size: wgpu::Extent3d {
                width: texture_width as _,
                height: texture_height as _,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let texture_id = renderer.write().register_native_texture(
            device,
            &texture.create_view(&wgpu::TextureViewDescriptor::default()),
            wgpu::FilterMode::Nearest,
        );

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Texture Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                }],
            });

        let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Texture Bind Group"),
            layout: &texture_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(
                    &texture.create_view(&wgpu::TextureViewDescriptor::default()),
                ),
            }],
        });

        let camera_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera Uniform Buffer"),
            size: <GpuCamera as ShaderSize>::SHADER_SIZE.get(),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
            mapped_at_creation: false,
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Camera Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: Some(<GpuCamera as ShaderSize>::SHADER_SIZE),
                    },
                    count: None,
                }],
            });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera Bind Group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &camera_uniform_buffer,
                    offset: 0,
                    size: Some(<GpuCamera as ShaderSize>::SHADER_SIZE),
                }),
            }],
        });

        let hyper_spheres_storage_buffer_size =
            <GpuHyperSpheres as ShaderType>::min_size().get() as usize;
        let hyper_spheres_storage_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Hyper Spheres Storage Buffer"),
            size: hyper_spheres_storage_buffer_size as _,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        let objects_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Objects Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: Some(<GpuHyperSpheres as ShaderType>::min_size()),
                    },
                    count: None,
                }],
            });

        let objects_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Objects Bind Group"),
            layout: &objects_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &hyper_spheres_storage_buffer,
                    offset: 0,
                    size: None,
                }),
            }],
        });

        let ray_tracing_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Ray Tracing Pipeline Layout"),
                bind_group_layouts: &[
                    &texture_bind_group_layout,
                    &camera_bind_group_layout,
                    &objects_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });
        let ray_tracing_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Ray Tracing Pipeline"),
                layout: Some(&ray_tracing_pipeline_layout),
                module: &ray_tracing_shader,
                entry_point: "ray_trace",
            });

        Self {
            previous_time: std::time::Instant::now(),
            texture_width,
            texture_height,
            texture_id,
            texture_bind_group_layout,
            texture_bind_group,
            camera: Camera {
                position: cgmath::vec4(0.0, 0.0, -3.0, 0.0),
                pitch: 0.0,
                yaw: 0.0,
                weird_pitch: 0.0,
                weird_yaw: 0.0,
                fov: 90.0f32.to_radians(),
                min_distance: 0.01,
                max_distance: 1000.0,
            },
            camera_uniform_buffer,
            camera_bind_group,
            hyper_spheres: vec![GpuHyperSphere {
                center: cgmath::vec4(0.0, 0.0, 0.0, 0.0),
                radius: 1.0,
            }],
            hyper_sphere_names: vec!["Hyper Sphere".into()],
            hyper_spheres_storage_buffer,
            hyper_spheres_storage_buffer_size,
            objects_bind_group_layout,
            objects_bind_group,
            ray_tracing_pipeline,
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let time = std::time::Instant::now();
        let dt = time.duration_since(self.previous_time);

        let ts = dt.as_secs_f32();

        let camera_rotation = Rotor4::from_angle_plane(self.camera.yaw, BiVector4::ZX)
            .rotate_by(Rotor4::from_angle_plane(self.camera.pitch, BiVector4::ZY))
            .rotate_by(Rotor4::from_angle_plane(
                self.camera.weird_yaw,
                BiVector4::WX,
            ))
            .rotate_by(Rotor4::from_angle_plane(
                self.camera.weird_pitch,
                BiVector4::WZ,
            ));
        let camera_forward = camera_rotation.rotate_vec(cgmath::vec4(0.0, 0.0, 1.0, 0.0));
        let camera_right = camera_rotation.rotate_vec(cgmath::vec4(1.0, 0.0, 0.0, 0.0));
        let camera_up = camera_rotation.rotate_vec(cgmath::vec4(0.0, 1.0, 0.0, 0.0));

        egui::SidePanel::left("Left Panel").show(ctx, |ui| {
            #[inline(always)]
            fn edit_value(
                ui: &mut egui::Ui,
                label: impl Into<egui::WidgetText>,
                value: &mut impl egui::emath::Numeric,
            ) {
                ui.horizontal(|ui| {
                    ui.label(label);
                    ui.add(egui::DragValue::new(value).speed(0.01));
                });
            }

            #[inline(always)]
            fn edit_vec4(
                ui: &mut egui::Ui,
                label: impl Into<egui::WidgetText>,
                vec: &mut cgmath::Vector4<impl egui::emath::Numeric>,
            ) {
                ui.horizontal(|ui| {
                    ui.label(label);
                    ui.add(egui::DragValue::new(&mut vec.x).prefix("x: ").speed(0.01));
                    ui.add(egui::DragValue::new(&mut vec.y).prefix("y: ").speed(0.01));
                    ui.add(egui::DragValue::new(&mut vec.z).prefix("z: ").speed(0.01));
                    ui.add(egui::DragValue::new(&mut vec.w).prefix("w: ").speed(0.01));
                });
            }

            ui.collapsing("Camera", |ui| {
                edit_vec4(ui, "Position: ", &mut self.camera.position);

                ui.horizontal(|ui| {
                    ui.label("Fov: ");
                    ui.drag_angle(&mut self.camera.fov);
                });

                edit_value(ui, "Min Distance: ", &mut self.camera.min_distance);
                self.camera.min_distance = self.camera.min_distance.max(0.0);

                edit_value(ui, "Max Distance: ", &mut self.camera.max_distance);
                self.camera.max_distance = self.camera.max_distance.max(self.camera.min_distance);

                ui.horizontal(|ui| {
                    ui.label("Pitch: ");
                    ui.drag_angle(&mut self.camera.pitch);
                });
                ui.horizontal(|ui| {
                    ui.label("Yaw: ");
                    ui.drag_angle(&mut self.camera.yaw);
                });
                ui.horizontal(|ui| {
                    ui.label("4D Pitch: ");
                    ui.drag_angle(&mut self.camera.weird_pitch);
                });
                ui.horizontal(|ui| {
                    ui.label("4D Yaw: ");
                    ui.drag_angle(&mut self.camera.weird_yaw);
                });

                ui.add_enabled_ui(false, |ui| {
                    edit_vec4(ui, "Forward: ", &mut camera_forward.clone());
                    edit_vec4(ui, "Right: ", &mut camera_right.clone());
                    edit_vec4(ui, "Up: ", &mut camera_up.clone());
                });
            });
            ui.collapsing("Hyper Spheres", |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (i, (hyper_sphere, name)) in self
                        .hyper_spheres
                        .iter_mut()
                        .zip(self.hyper_sphere_names.iter_mut())
                        .enumerate()
                    {
                        egui::CollapsingHeader::new(name.as_str())
                            .id_source(i)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label("Name: ");
                                    ui.text_edit_singleline(name);
                                });
                                edit_vec4(ui, "Center: ", &mut hyper_sphere.center);
                                edit_value(ui, "Radius: ", &mut hyper_sphere.radius);
                            });
                    }
                });
            });
            ui.allocate_space(ui.available_size());
        });

        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(ctx.style().visuals.panel_fill))
            .show(ctx, |ui| {
                let eframe::egui_wgpu::RenderState {
                    device,
                    queue,
                    renderer,
                    ..
                } = frame.wgpu_render_state().unwrap();

                let size = ui.available_size();
                let size = (size.x.max(1.0) as usize, size.y.max(1.0) as usize);

                // recreate the texture if it is the wrong size
                if size != (self.texture_width, self.texture_height) {
                    (self.texture_width, self.texture_height) = size;

                    let texture = device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("Texture"),
                        size: wgpu::Extent3d {
                            width: self.texture_width as _,
                            height: self.texture_height as _,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        usage: wgpu::TextureUsages::STORAGE_BINDING
                            | wgpu::TextureUsages::TEXTURE_BINDING,
                        view_formats: &[],
                    });

                    self.texture_bind_group =
                        device.create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("Texture Bind Group"),
                            layout: &self.texture_bind_group_layout,
                            entries: &[wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::TextureView(
                                    &texture.create_view(&wgpu::TextureViewDescriptor::default()),
                                ),
                            }],
                        });

                    renderer.write().update_egui_texture_from_wgpu_texture(
                        device,
                        &texture.create_view(&wgpu::TextureViewDescriptor::default()),
                        wgpu::FilterMode::Nearest,
                        self.texture_id,
                    );
                }

                // Upload camera
                {
                    let mut camera_buffer =
                        UniformBuffer::new([0; <GpuCamera as ShaderSize>::SHADER_SIZE.get() as _]);
                    camera_buffer
                        .write(&GpuCamera {
                            position: self.camera.position,
                            forward: camera_forward,
                            right: camera_right,
                            up: camera_up,
                            fov: self.camera.fov,
                            min_distance: self.camera.min_distance,
                            max_distance: self.camera.max_distance,
                        })
                        .unwrap();
                    let camera_buffer = camera_buffer.into_inner();

                    queue.write_buffer(&self.camera_uniform_buffer, 0, &camera_buffer);
                }

                // Upload objects
                {
                    let mut bind_group_invalidated = false;

                    // Upload hyper spheres
                    {
                        let mut hyper_spheres_buffer = DynamicStorageBuffer::new(vec![]);
                        hyper_spheres_buffer
                            .write(&GpuHyperSpheres {
                                count: ArrayLength,
                                data: &self.hyper_spheres,
                            })
                            .unwrap();
                        let hyper_spheres_buffer = hyper_spheres_buffer.into_inner();

                        if hyper_spheres_buffer.len() <= self.hyper_spheres_storage_buffer_size {
                            queue.write_buffer(
                                &self.hyper_spheres_storage_buffer,
                                0,
                                &hyper_spheres_buffer,
                            );
                        } else {
                            self.hyper_spheres_storage_buffer =
                                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                    label: Some("Hyper Spheres Storage Buffer"),
                                    contents: &hyper_spheres_buffer,
                                    usage: wgpu::BufferUsages::COPY_DST
                                        | wgpu::BufferUsages::STORAGE,
                                });
                            self.hyper_spheres_storage_buffer_size = hyper_spheres_buffer.len();
                            bind_group_invalidated = true;
                        }
                    }

                    if bind_group_invalidated {
                        self.objects_bind_group =
                            device.create_bind_group(&wgpu::BindGroupDescriptor {
                                label: Some("Objects Bind Group"),
                                layout: &self.objects_bind_group_layout,
                                entries: &[wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                        buffer: &self.hyper_spheres_storage_buffer,
                                        offset: 0,
                                        size: None,
                                    }),
                                }],
                            });
                    }
                }

                // do the ray tracing
                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Compute Command Encoder"),
                });
                {
                    let workgroup_size = (16, 16);
                    let (dispatch_width, dispatch_height) = (
                        (self.texture_width + workgroup_size.0 - 1) / workgroup_size.0,
                        (self.texture_height + workgroup_size.1 - 1) / workgroup_size.1,
                    );

                    let mut compute_pass =
                        encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                            label: Some("Compute Pass"),
                        });
                    compute_pass.set_pipeline(&self.ray_tracing_pipeline);
                    compute_pass.set_bind_group(0, &self.texture_bind_group, &[]);
                    compute_pass.set_bind_group(1, &self.camera_bind_group, &[]);
                    compute_pass.set_bind_group(2, &self.objects_bind_group, &[]);
                    compute_pass.dispatch_workgroups(dispatch_width as _, dispatch_height as _, 1);
                }
                queue.submit([encoder.finish()]);

                ui.image(
                    self.texture_id,
                    egui::vec2(self.texture_width as _, self.texture_height as _),
                );
            });

        if !ctx.wants_keyboard_input() {
            ctx.input(|i| {
                const CAMERA_SPEED: f32 = 3.0;
                let camera_rotation_speed: f32 = 90.0f32.to_radians() * 1.5;

                if i.key_down(egui::Key::W) {
                    self.camera.position += camera_forward * (CAMERA_SPEED * ts);
                }
                if i.key_down(egui::Key::S) {
                    self.camera.position -= camera_forward * (CAMERA_SPEED * ts);
                }
                if i.key_down(egui::Key::A) {
                    self.camera.position -= camera_right * (CAMERA_SPEED * ts);
                }
                if i.key_down(egui::Key::D) {
                    self.camera.position += camera_right * (CAMERA_SPEED * ts);
                }
                if i.key_down(egui::Key::Q) {
                    self.camera.position -= camera_up * (CAMERA_SPEED * ts);
                }
                if i.key_down(egui::Key::E) {
                    self.camera.position += camera_up * (CAMERA_SPEED * ts);
                }

                if i.modifiers.shift {
                    if i.key_down(egui::Key::ArrowUp) {
                        self.camera.weird_pitch += camera_rotation_speed * ts;
                    }
                    if i.key_down(egui::Key::ArrowDown) {
                        self.camera.weird_pitch -= camera_rotation_speed * ts;
                    }
                    if i.key_down(egui::Key::ArrowLeft) {
                        self.camera.weird_yaw -= camera_rotation_speed * ts;
                    }
                    if i.key_down(egui::Key::ArrowRight) {
                        self.camera.weird_yaw += camera_rotation_speed * ts;
                    }
                } else {
                    if i.key_down(egui::Key::ArrowUp) {
                        self.camera.pitch += camera_rotation_speed * ts;
                    }
                    if i.key_down(egui::Key::ArrowDown) {
                        self.camera.pitch -= camera_rotation_speed * ts;
                    }
                    if i.key_down(egui::Key::ArrowLeft) {
                        self.camera.yaw -= camera_rotation_speed * ts;
                    }
                    if i.key_down(egui::Key::ArrowRight) {
                        self.camera.yaw += camera_rotation_speed * ts;
                    }
                }
            });
        }

        ctx.request_repaint();
        self.previous_time = time;
    }
}
