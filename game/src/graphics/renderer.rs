use miniquad::*;
// TODO(jhurstwright): Replace with no_std hashmap
use std::collections::HashMap;

use crate::components;
use crate::graphics;
use crate::graphics::font;
use crate::shaders;
use crate::utils;

pub type Materials = HashMap<AssetIdentity, MaterialAsset>;
pub type Meshes = HashMap<AssetIdentity, MeshAsset>;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct AssetIdentity(pub String);

impl From<String> for AssetIdentity {
    fn from(v: String) -> Self {
        Self(v)
    }
}

impl From<&'_ str> for AssetIdentity {
    fn from(v: &'_ str) -> Self {
        Self(v.to_owned())
    }
}

#[derive(Debug, Clone)]
pub struct SpriteRenderCommand {
    pub binding: AssetIdentity,
    pub position: glam::Vec2,
    pub angle: f32,
    pub num_of_elements: i32,
}

#[derive(Debug, Clone)]
pub struct RenderFontCommand {
    pub font: String,
    pub text: String,
    pub position: glam::Vec2,
}

#[derive(Debug, Clone)]
pub enum RenderAssetCommands {
    LoadText {
        text: String,
        font: String,
    },
    // TOOD(jhurstwright): I really want to blindly create, and GC old texts later
    UpdateText {
        new_text: String,
        text: String,
        font: String,
    },
}

#[derive(Debug, Clone)]
pub struct MeshAsset {
    pub identity: AssetIdentity,
    pub vertices: Vec<miniquad::Buffer>,
    pub indices: miniquad::Buffer,
    pub num_of_indices: u16,
}

impl MeshAsset {
    pub fn new<T: Into<AssetIdentity>>(
        identity: T,
        vertices: Vec<miniquad::Buffer>,
        indices: miniquad::Buffer,
        num_of_indices: u16,
    ) -> Self {
        Self {
            identity: identity.into(),
            vertices,
            indices,
            num_of_indices,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MaterialAsset {
    pub identity: AssetIdentity,
    pub textures: Vec<miniquad::Texture>,
}

impl MaterialAsset {
    pub fn new<T: Into<AssetIdentity>>(identity: T, textures: Vec<miniquad::Texture>) -> Self {
        Self {
            identity: identity.into(),
            textures,
        }
    }
}

pub struct MainRenderer {
    pub debug_font_bindings: miniquad::Bindings,
    pub shader_pipeline: miniquad::Pipeline,
    // TODO(jhurstwright): These should be consolidated into a UnionEnum
    pub render_font_commands: Vec<RenderFontCommand>,
    pub render_commands: Vec<SpriteRenderCommand>,
    pub asset_commands: Vec<RenderAssetCommands>,
    pub fonts: HashMap<String, font::Font>,
    pub texts: HashMap<String, (Vec<miniquad::Buffer>, miniquad::Buffer)>,
    pub meshes: Meshes,
    pub materials: Materials,
    pub projection: glam::Mat4,
    pub view: glam::Mat4,
}

fn create_text_buffer(
    renderer: &mut graphics::MainRenderer,
    ctx: &mut Context,
    text: String,
    font: &String,
) -> Option<(Vec<miniquad::Buffer>, miniquad::Buffer)> {
    let font = match renderer.fonts.get(font) {
        Some(f) => f,
        _ => return None,
    };
    use crate::shaders::Vertex;
    use glam::Vec2;
    let mut vertices: Vec<Vertex> = Vec::with_capacity(text.chars().count() * 4);
    let mut indices: Vec<u16> = Vec::with_capacity(text.chars().count() * 6);
    let (width, height) = font.image_dimensions();
    let mut offset = 0.0f32;
    let scale = 0.025f32;
    for (index, character) in text.chars().enumerate() {
        let index = index as u16;
        if let Some(glyph) = font.glyphs.get(&character) {
            let font::CharInfo {
                glyph_x,
                glyph_y,
                glyph_h,
                glyph_w,
                ..
            } = *glyph;
            let w = (glyph_w as f32 / 2.) * scale;
            let h = (glyph_h as f32 / 2.) * scale;
            let texture_x = glyph_x as f32 / width as f32;
            let texture_y = glyph_y as f32 / height as f32;
            let texture_w = glyph_w as f32 / width as f32;
            let texture_h = glyph_h as f32 / height as f32;

            vertices.push(Vertex {
                pos: Vec2::new(offset - w, -h),
                uv: Vec2::new(texture_x, texture_y + texture_h),
            });
            vertices.push(Vertex {
                pos: Vec2::new(offset + w, -h),
                uv: Vec2::new(texture_x + texture_w, texture_y + texture_h),
            });
            vertices.push(Vertex {
                pos: Vec2::new(offset + w, h),
                uv: Vec2::new(texture_x + texture_w, texture_y),
            });
            vertices.push(Vertex {
                pos: Vec2::new(offset - w, h),
                uv: Vec2::new(texture_x, texture_y),
            });

            indices.push(0 + (index * 4));
            indices.push(1 + (index * 4));
            indices.push(2 + (index * 4));
            indices.push(0 + (index * 4));
            indices.push(2 + (index * 4));
            indices.push(3 + (index * 4));

            offset += glyph.advance * scale;
        }
    }
    let vertex_buffer = Buffer::immutable(ctx, BufferType::VertexBuffer, &vertices);
    let index_buffer = Buffer::immutable(ctx, BufferType::IndexBuffer, &indices);
    Some((vec![vertex_buffer], index_buffer))
}

impl MainRenderer {
    pub fn new(ctx: &mut Context) -> Self {
        let shader = shaders::sprite::new(ctx).unwrap();
        let shader_pipeline = Pipeline::with_params(
            ctx,
            &[BufferLayout::default()],
            &[
                VertexAttribute::new("pos", VertexFormat::Float2),
                VertexAttribute::new("uv", VertexFormat::Float2),
            ],
            shader,
            PipelineParams {
                color_blend: Some(BlendState::new(
                    miniquad::Equation::Add,
                    miniquad::BlendFactor::Value(BlendValue::SourceAlpha),
                    BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                )),
                ..Default::default()
            },
        );

        let mut materials = HashMap::new();
        let mut meshes = HashMap::new();

        let snake_texture =
            crate::utils::build_square_texture(ctx, 4, crate::graphics::colors::RAYWHITE);
        let tail_texture =
            crate::utils::build_square_texture(ctx, 4, crate::graphics::colors::RAYWHITE);
        let food_texture =
            crate::utils::build_square_texture(ctx, 4, crate::graphics::colors::PURPLE);
        let arrow_texture =
            crate::utils::build_square_texture(ctx, 4, crate::graphics::colors::RED);

        let mut fonts = HashMap::new();

        materials.insert(
            "Food".into(),
            MaterialAsset::new("Food", vec![food_texture]),
        );
        materials.insert(
            "Tail".into(),
            MaterialAsset::new("Tail", vec![tail_texture]),
        );
        materials.insert(
            "Snake".into(),
            MaterialAsset::new("Snake", vec![snake_texture]),
        );
        materials.insert(
            "Arrow".into(),
            MaterialAsset::new("Arrow", vec![arrow_texture]),
        );

        let snake_mesh = crate::utils::make_square(ctx, 1.);
        let food_mesh = crate::utils::make_square(ctx, 0.8);
        let tail_mesh = crate::utils::make_square(ctx, 0.8);
        let arrow_mesh = crate::utils::make_arrow(ctx);

        meshes.insert(
            "Food".into(),
            MeshAsset::new("Food", vec![food_mesh.0], food_mesh.1, food_mesh.2),
        );
        meshes.insert(
            "Tail".into(),
            MeshAsset::new("Tail", vec![tail_mesh.0], tail_mesh.1, tail_mesh.2),
        );
        meshes.insert(
            "Snake".into(),
            MeshAsset::new("Snake", vec![snake_mesh.0], snake_mesh.1, snake_mesh.2),
        );
        meshes.insert(
            "Arrow".into(),
            MeshAsset::new("Arrow", vec![arrow_mesh.0], arrow_mesh.1, arrow_mesh.2),
        );

        let mut fallback_font = font::Font::load("KenneyFuture", include_bytes!("KenneyFuture.ttf"));
        for char in font::ascii_character_list() {
            fallback_font.cache_glyph(char);
        }
        let tex = fallback_font.texture(ctx);
        let (vertices, indices, _) = utils::make_square(ctx, 32.);
        let bindings = miniquad::Bindings {
            vertex_buffers: vec![vertices],
            index_buffer: indices,
            images: vec![tex],
        };
        fonts.insert(fallback_font.name.clone(), fallback_font);

        Self {
            asset_commands: Vec::with_capacity(32),
            debug_font_bindings: bindings,
            fonts,
            materials,
            meshes,
            texts: HashMap::new(),
            projection: glam::Mat4::identity(),
            render_font_commands: Vec::with_capacity(64),
            render_commands: Vec::with_capacity(64),
            shader_pipeline,
            view: glam::Mat4::identity(),
        }
    }

    pub fn update_view(&mut self, camera: &components::Camera2D) {
        self.projection = camera.projection;
        self.view = camera.view;
    }

    pub fn load_assets(&mut self, ctx: &mut Context) {
        let commands: Vec<RenderAssetCommands> = self.asset_commands.drain(..).collect();
        commands.iter().for_each(|cmd| match cmd {
            RenderAssetCommands::LoadText { text, font } => {
                if !self.texts.contains_key(text) {
                    let buffer = create_text_buffer(self, ctx, text.clone(), font);
                    if let Some(buffers) = buffer {
                        self.texts.insert(text.clone(), buffers);
                    }
                }
            }
            RenderAssetCommands::UpdateText {
                text,
                font,
                new_text,
            } => {
                if let Some((vertices, indices)) = self.texts.remove(text) {
                    vertices.iter().for_each(|b| b.delete());
                    indices.delete();
                }
                let buffer = create_text_buffer(self, ctx, new_text.clone(), font);
                if let Some(buffers) = buffer {
                    self.texts.insert(new_text.clone(), buffers);
                }
            }
        });
    }

    pub fn draw(&mut self, ctx: &mut Context) {
        ctx.begin_default_pass(PassAction::Clear {
            color: Some(graphics::colors::DARKGRAY.into()),
            depth: Some(1.),
            stencil: None,
        });

        let mut uniform = crate::shaders::sprite::VertexUniforms {
            projection: self.projection,
            view: self.view,
            model: glam::Mat4::identity(),
        };

        ctx.apply_pipeline(&self.shader_pipeline);
        {
            for SpriteRenderCommand {
                position,
                binding,
                num_of_elements,
                angle,
            } in self.render_commands.iter()
            {
                let mesh = match self.meshes.get(binding) {
                    Some(m) => m,
                    _ => continue,
                };
                let material = match self.materials.get(binding) {
                    Some(m) => m,
                    _ => continue,
                };
                let model = glam::Mat4::from_rotation_translation(
                    glam::Quat::from_axis_angle(glam::Vec3::new(0., 0., 1.), *angle),
                    glam::Vec3::new(position.x, position.y, 0.),
                );
                uniform.model = model;
                let bindings = miniquad::Bindings {
                    vertex_buffers: mesh.vertices.clone(),
                    index_buffer: mesh.indices.clone(),
                    images: material.textures.clone(),
                };
                ctx.apply_bindings(&bindings);
                ctx.apply_uniforms(&uniform);
                ctx.draw(0, *num_of_elements, 1);
            }
        }

        // Show how the text is Rendered
        // TODO(jhurstwright): I still want to put this into the Debug UI
        // {
        //     let model = glam::Mat4::from_rotation_translation(
        //         glam::Quat::from_axis_angle(glam::Vec3::new(0., 0., 1.), (0.0f32).to_radians()),
        //         glam::Vec3::new(10., 0., 0.),
        //     );
        //     uniform.model = model;
        //     ctx.apply_bindings(&self.debug_font_bindings);
        //     ctx.apply_uniforms(&uniform);
        //     ctx.draw(0, 6, 1);
        // }

        // Render the Font
        for cmd in self.render_font_commands.iter() {
            let RenderFontCommand { text, position, .. } = cmd;
            if let Some((v, i)) = &self.texts.get(text) {
                let model = glam::Mat4::from_rotation_translation(
                    glam::Quat::from_axis_angle(glam::Vec3::new(0., 0., 1.), (0.0f32).to_radians()),
                    glam::Vec3::new(position.x, position.y, 0.),
                );
                let m = &self.debug_font_bindings.images;
                uniform.model = model;
                let bindings = miniquad::Bindings {
                    vertex_buffers: v.clone(),
                    index_buffer: i.clone(),
                    images: m.clone(),
                };
                ctx.apply_bindings(&bindings);
                ctx.apply_uniforms(&uniform);
                ctx.draw(0, 6 * text.len() as i32, 1);
            }
        }
        ctx.end_render_pass();
        ctx.commit_frame();
        self.render_commands.clear();
    }
}
