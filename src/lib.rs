use cgmath::prelude::*;
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
    pub bounce_count: u32,
    pub sample_count: u32,
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
    pub bounce_count: u32,
    pub sample_count: u32,
}

#[derive(Clone, Copy, ShaderType)]
struct GpuHyperSphere {
    pub center: cgmath::Vector4<f32>,
    pub radius: f32,
    pub material: u32,
}

#[derive(Clone, Copy, ShaderType)]
struct GpuHyperSpheres<'a> {
    pub count: ArrayLength,
    #[size(runtime)]
    pub data: &'a [GpuHyperSphere],
}

#[derive(Clone, Copy, ShaderType)]
struct GpuHyperPlane {
    pub point: cgmath::Vector4<f32>,
    pub normal: cgmath::Vector4<f32>,
    pub material: u32,
}

#[derive(Clone, Copy, ShaderType)]
struct GpuHyperPlanes<'a> {
    pub count: ArrayLength,
    #[size(runtime)]
    pub data: &'a [GpuHyperPlane],
}

#[derive(Clone, Copy, ShaderType)]
struct GpuMaterial {
    pub base_color: cgmath::Vector3<f32>,
    pub emissive_color: cgmath::Vector3<f32>,
    pub emission_strength: f32,
}

#[derive(Clone, Copy, ShaderType)]
struct GpuMaterials<'a> {
    pub count: ArrayLength,
    #[size(runtime)]
    pub data: &'a [GpuMaterial],
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
    hyper_planes: Vec<GpuHyperPlane>,
    hyper_plane_names: Vec<String>,
    hyper_planes_storage_buffer: wgpu::Buffer,
    hyper_planes_storage_buffer_size: usize,
    objects_bind_group_layout: wgpu::BindGroupLayout,
    objects_bind_group: wgpu::BindGroup,
    materials: Vec<GpuMaterial>,
    materials_storage_buffer: wgpu::Buffer,
    materials_storage_buffer_size: usize,
    materials_bind_group_layout: wgpu::BindGroupLayout,
    materials_bind_group: wgpu::BindGroup,
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

        let hyper_planes_storage_buffer_size =
            <GpuHyperPlanes as ShaderType>::min_size().get() as usize;
        let hyper_planes_storage_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Hyper Planes Storage Buffer"),
            size: hyper_planes_storage_buffer_size as _,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        let objects_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Objects Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: Some(<GpuHyperSpheres as ShaderType>::min_size()),
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: Some(<GpuHyperPlanes as ShaderType>::min_size()),
                        },
                        count: None,
                    },
                ],
            });

        let objects_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Objects Bind Group"),
            layout: &objects_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &hyper_spheres_storage_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &hyper_planes_storage_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
            ],
        });

        let materials_storage_buffer_size = <GpuMaterials as ShaderType>::min_size().get() as usize;
        let materials_storage_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Materials Storage Buffer"),
            size: materials_storage_buffer_size as _,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        let materials_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Materials Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: Some(<GpuMaterials as ShaderType>::min_size()),
                    },
                    count: None,
                }],
            });

        let materials_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Materials Bind Group"),
            layout: &materials_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &materials_storage_buffer,
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
                    &materials_bind_group_layout,
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
                position: cgmath::vec4(0.0, 1.0, -3.0, 0.0),
                pitch: 0.0,
                yaw: 0.0,
                weird_pitch: 0.0,
                weird_yaw: 0.0,
                fov: 90.0f32.to_radians(),
                min_distance: 0.01,
                max_distance: 1000.0,
                bounce_count: 5,
                sample_count: 1,
            },
            camera_uniform_buffer,
            camera_bind_group,
            hyper_spheres: vec![GpuHyperSphere {
                center: cgmath::vec4(0.0, 1.0, 0.0, 0.0),
                radius: 1.0,
                material: 0,
            }],
            hyper_sphere_names: vec!["Hyper Sphere".into()],
            hyper_spheres_storage_buffer,
            hyper_spheres_storage_buffer_size,
            hyper_planes: vec![GpuHyperPlane {
                point: cgmath::vec4(0.0, 0.0, 0.0, 0.0),
                normal: cgmath::vec4(0.0, 1.0, 0.0, 0.0),
                material: 1,
            }],
            hyper_plane_names: vec!["Ground".into()],
            hyper_planes_storage_buffer,
            hyper_planes_storage_buffer_size,
            objects_bind_group_layout,
            objects_bind_group,
            materials: vec![
                GpuMaterial {
                    base_color: cgmath::vec3(0.8, 0.4, 0.1),
                    emissive_color: cgmath::vec3(0.0, 0.0, 0.0),
                    emission_strength: 0.0,
                },
                GpuMaterial {
                    base_color: cgmath::vec3(0.1, 0.8, 0.3),
                    emissive_color: cgmath::vec3(0.0, 0.0, 0.0),
                    emission_strength: 0.0,
                },
            ],
            materials_storage_buffer,
            materials_storage_buffer_size,
            materials_bind_group_layout,
            materials_bind_group,
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
                BiVector4::XW,
            ))
            .rotate_by(Rotor4::from_angle_plane(
                self.camera.weird_pitch,
                BiVector4::ZW,
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
                speed: impl Into<f64>,
            ) {
                ui.horizontal(|ui| {
                    ui.label(label);
                    ui.add(egui::DragValue::new(value).speed(speed));
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

            #[inline(always)]
            fn edit_angle(ui: &mut egui::Ui, label: impl Into<egui::WidgetText>, angle: &mut f32) {
                ui.horizontal(|ui| {
                    ui.label(label);
                    ui.drag_angle(angle);
                });
                *angle %= std::f32::consts::TAU;
                *angle += std::f32::consts::TAU;
                *angle %= std::f32::consts::TAU;
            }

            #[inline(always)]
            fn edit_color3(
                ui: &mut egui::Ui,
                label: impl Into<egui::WidgetText>,
                color: &mut cgmath::Vector3<f32>,
            ) {
                ui.horizontal(|ui| {
                    ui.label(label);
                    let mut array = [color.x, color.y, color.z];
                    egui::color_picker::color_edit_button_rgb(ui, &mut array);
                    *color = cgmath::vec3(array[0], array[1], array[2]);
                });
            }

            #[inline(always)]
            fn edit_material(ui: &mut egui::Ui, material: &mut GpuMaterial) {
                ui.collapsing("Material", |ui| {
                    edit_color3(ui, "Base Color: ", &mut material.base_color);
                    edit_color3(ui, "Emissive Color: ", &mut material.emissive_color);
                    edit_value(
                        ui,
                        "Emissive Strength: ",
                        &mut material.emission_strength,
                        0.01,
                    );
                });
            }

            ui.collapsing("Camera", |ui| {
                edit_vec4(ui, "Position: ", &mut self.camera.position);
                edit_angle(ui, "Fov: ", &mut self.camera.fov);
                edit_value(ui, "Min Distance: ", &mut self.camera.min_distance, 0.01);
                self.camera.min_distance = self.camera.min_distance.max(0.0);
                edit_value(ui, "Max Distance: ", &mut self.camera.max_distance, 0.01);
                self.camera.max_distance = self.camera.max_distance.max(self.camera.min_distance);
                edit_angle(ui, "Pitch: ", &mut self.camera.pitch);
                edit_angle(ui, "Yaw: ", &mut self.camera.yaw);
                edit_angle(ui, "4D Pitch: ", &mut self.camera.weird_pitch);
                edit_angle(ui, "4D Yaw: ", &mut self.camera.weird_yaw);
                edit_value(ui, "Max Bounces: ", &mut self.camera.bounce_count, 1);
                self.camera.bounce_count = self.camera.bounce_count.max(1);
                edit_value(ui, "Sample Count: ", &mut self.camera.sample_count, 1);
                self.camera.sample_count = self.camera.sample_count.max(1);
                ui.add_enabled_ui(false, |ui| {
                    edit_vec4(ui, "Forward: ", &mut camera_forward.clone());
                    edit_vec4(ui, "Right: ", &mut camera_right.clone());
                    edit_vec4(ui, "Up: ", &mut camera_up.clone());
                });
            });
            ui.collapsing("Hyper Spheres", |ui| {
                if ui.button("Add Hyper Sphere").clicked() {
                    let material = self.materials.len() as u32;
                    self.materials.push(GpuMaterial {
                        base_color: cgmath::vec3(0.9, 0.9, 0.9),
                        emissive_color: cgmath::vec3(0.0, 0.0, 0.0),
                        emission_strength: 0.0,
                    });

                    self.hyper_spheres.push(GpuHyperSphere {
                        center: cgmath::vec4(0.0, 0.0, 0.0, 0.0),
                        radius: 1.0,
                        material,
                    });
                    self.hyper_sphere_names.push("Default Hyper Sphere".into());
                }

                let mut to_delete = vec![];
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
                                edit_value(ui, "Radius: ", &mut hyper_sphere.radius, 0.01);
                                edit_material(
                                    ui,
                                    &mut self.materials[hyper_sphere.material as usize],
                                );
                                if ui.button("Delete").clicked() {
                                    to_delete.push(i);
                                }
                            });
                    }
                });
                for i in to_delete {
                    self.hyper_spheres.remove(i);
                    self.hyper_sphere_names.remove(i);
                }
            });
            ui.collapsing("Hyper Planes", |ui| {
                if ui.button("Add Hyper Plane").clicked() {
                    let material = self.materials.len() as u32;
                    self.materials.push(GpuMaterial {
                        base_color: cgmath::vec3(0.9, 0.9, 0.9),
                        emissive_color: cgmath::vec3(0.0, 0.0, 0.0),
                        emission_strength: 0.0,
                    });

                    self.hyper_planes.push(GpuHyperPlane {
                        point: cgmath::vec4(0.0, 0.0, 0.0, 0.0),
                        normal: cgmath::vec4(0.0, 1.0, 0.0, 0.0),
                        material,
                    });
                    self.hyper_plane_names.push("Default Hyper Plane".into());
                }

                let mut to_delete = vec![];
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (i, (hyper_plane, name)) in self
                        .hyper_planes
                        .iter_mut()
                        .zip(self.hyper_plane_names.iter_mut())
                        .enumerate()
                    {
                        egui::CollapsingHeader::new(name.as_str())
                            .id_source(i)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label("Name: ");
                                    ui.text_edit_singleline(name);
                                });
                                edit_vec4(ui, "Point: ", &mut hyper_plane.point);
                                edit_vec4(ui, "Normal: ", &mut hyper_plane.normal);
                                hyper_plane.normal = hyper_plane.normal.normalize();
                                edit_material(
                                    ui,
                                    &mut self.materials[hyper_plane.material as usize],
                                );
                                if ui.button("Delete").clicked() {
                                    to_delete.push(i);
                                }
                            });
                    }
                });
                for i in to_delete {
                    self.hyper_planes.remove(i);
                    self.hyper_plane_names.remove(i);
                }
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
                            bounce_count: self.camera.bounce_count,
                            sample_count: self.camera.sample_count,
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

                    // Upload Hyper Planes
                    {
                        let mut hyper_planes_buffer = DynamicStorageBuffer::new(vec![]);
                        hyper_planes_buffer
                            .write(&GpuHyperPlanes {
                                count: ArrayLength,
                                data: &self.hyper_planes,
                            })
                            .unwrap();
                        let hyper_planes_buffer = hyper_planes_buffer.into_inner();

                        if hyper_planes_buffer.len() <= self.hyper_planes_storage_buffer_size {
                            queue.write_buffer(
                                &self.hyper_planes_storage_buffer,
                                0,
                                &hyper_planes_buffer,
                            );
                        } else {
                            self.hyper_planes_storage_buffer =
                                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                    label: Some("Hyper Planes Storage Buffer"),
                                    contents: &hyper_planes_buffer,
                                    usage: wgpu::BufferUsages::COPY_DST
                                        | wgpu::BufferUsages::STORAGE,
                                });
                            self.hyper_planes_storage_buffer_size = hyper_planes_buffer.len();
                            bind_group_invalidated = true;
                        }
                    }

                    if bind_group_invalidated {
                        self.objects_bind_group =
                            device.create_bind_group(&wgpu::BindGroupDescriptor {
                                label: Some("Objects Bind Group"),
                                layout: &self.objects_bind_group_layout,
                                entries: &[
                                    wgpu::BindGroupEntry {
                                        binding: 0,
                                        resource: wgpu::BindingResource::Buffer(
                                            wgpu::BufferBinding {
                                                buffer: &self.hyper_spheres_storage_buffer,
                                                offset: 0,
                                                size: None,
                                            },
                                        ),
                                    },
                                    wgpu::BindGroupEntry {
                                        binding: 1,
                                        resource: wgpu::BindingResource::Buffer(
                                            wgpu::BufferBinding {
                                                buffer: &self.hyper_planes_storage_buffer,
                                                offset: 0,
                                                size: None,
                                            },
                                        ),
                                    },
                                ],
                            });
                    }
                }

                // Upload the materials
                {
                    let mut materials_buffer = DynamicStorageBuffer::new(vec![]);
                    materials_buffer
                        .write(&GpuMaterials {
                            count: ArrayLength,
                            data: &self.materials,
                        })
                        .unwrap();
                    let materials_buffer = materials_buffer.into_inner();

                    if materials_buffer.len() <= self.materials_storage_buffer_size {
                        queue.write_buffer(&self.materials_storage_buffer, 0, &materials_buffer);
                    } else {
                        self.materials_storage_buffer =
                            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("Materials Storage Buffer"),
                                contents: &materials_buffer,
                                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::STORAGE,
                            });
                        self.materials_storage_buffer_size = materials_buffer.len();

                        self.materials_bind_group =
                            device.create_bind_group(&wgpu::BindGroupDescriptor {
                                label: Some("Materials Bind Group"),
                                layout: &self.materials_bind_group_layout,
                                entries: &[wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                        buffer: &self.materials_storage_buffer,
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
                    compute_pass.set_bind_group(3, &self.materials_bind_group, &[]);
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
