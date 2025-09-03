use bevy::prelude::*;
use bevy_spritesheet_animation::prelude::*;
use serde::Deserialize;
use std::fs;

pub const DEFAULT_FRAME_MS: u32 = 100;

pub struct PlayerAnimationsPlugin;

impl Plugin for PlayerAnimationsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerSpritesheet>()
            .add_systems(Startup, (load_player_spritesheet, register_player_animations).chain());
    }
}

#[derive(Debug, Deserialize, Clone)]
struct AnimationEntry {
    name: String,
    row: usize,
    #[serde(rename = "frame_count", default)]
    _frame_count: usize,
    last_col: usize,
}

#[derive(Debug, Deserialize, Clone)]
struct SheetManifest {
    sheet_image: String,
    columns: usize,
    rows: usize,
    frame_w: u32,
    frame_h: u32,
    animations: Vec<AnimationEntry>,
}

#[derive(Resource, Default)]
pub struct PlayerSpritesheet {
    pub image: Handle<Image>,
    pub layout: Handle<TextureAtlasLayout>,
    manifest: Option<SheetManifest>,
}
fn load_player_spritesheet(
    mut atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    assets: Res<AssetServer>,
    mut sheet: ResMut<PlayerSpritesheet>,
) {
    let json_path = "assets/PlayerSheet2.json";
    let json_text = fs::read_to_string(json_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", json_path, e));

    let manifest: SheetManifest =
        serde_json::from_str(&json_text).expect("PlayerSheet2.json malformed");
    sheet.image = assets.load(&manifest.sheet_image);
    let spritesheet = Spritesheet::new(manifest.columns, manifest.rows);
    sheet.layout = atlas_layouts.add(spritesheet.atlas_layout(manifest.frame_w, manifest.frame_h));

    sheet.manifest = Some(manifest);
}

pub fn to_anim_name(raw: &str) -> String {
    fn slug(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        let mut prev_us = false;
        for ch in s.chars() {
            let c = ch.to_ascii_lowercase();
            if c.is_ascii_alphanumeric() {
                out.push(c);
                prev_us = false;
            } else if !prev_us {
                out.push('_');
                prev_us = true;
            }
        }
        out.trim_matches('_').to_string()
    }
    let mut parts: Vec<String> = raw
        .replace(['\\', '/'], "/")
        .split('/')
        .filter(|s| !s.is_empty())
        .map(slug)
        .filter(|s| !s.is_empty())
        .collect();

    if parts.is_empty() {
        return "player:unnamed".to_string();
    }
    let last = parts.pop().unwrap();
    let mut prefix = String::from("player");
    if !parts.is_empty() {
        prefix.push('_');
        prefix.push_str(&parts.join("_"));
    }

    format!("{}:{}", prefix, last)
}
fn register_player_animations(
    mut library: ResMut<AnimationLibrary>,
    sheet: Res<PlayerSpritesheet>,
) {
    let Some(manifest) = &sheet.manifest else {
        warn!("PlayerSpritesheet manifest not loaded yet");
        return;
    };

    let spritesheet = Spritesheet::new(manifest.columns, manifest.rows);

    for a in &manifest.animations {
        let frames = if a.last_col + 1 == manifest.columns {
            spritesheet.row(a.row)
        } else {
            spritesheet.row_partial(a.row, 0..=a.last_col)
        };

        let clip = Clip::from_frames(frames)
            .with_duration(AnimationDuration::PerFrame(DEFAULT_FRAME_MS));

        let clip_id = library.register_clip(clip);
        let anim_id = library.register_animation(Animation::from_clip(clip_id));

        let pretty = to_anim_name(&a.name);
        let _ = library.name_animation(anim_id, &pretty);
        bevy::log::info!(
            "registered animation: {:<32} | row {:02} | frames 0..={}",
            pretty,
            a.row,
            a.last_col
        );
    }

    bevy::log::info!(
        "registered {} player animations from {} ({}x{} cells, frame {}x{})",
        manifest.animations.len(),
        manifest.sheet_image,
        manifest.columns,
        manifest.rows,
        manifest.frame_w,
        manifest.frame_h
    );
}