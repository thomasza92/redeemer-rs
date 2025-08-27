use bevy::prelude::*;
use bevy_spritesheet_animation::prelude::*;

pub struct PlayerAnimationsPlugin;

impl Plugin for PlayerAnimationsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerSpritesheet>()
            .add_systems(Startup, (load_player_spritesheet, register_player_animations));
    }
}

/// Handles for the player's spritesheet so other systems can spawn sprites.
#[derive(Resource, Default)]
pub struct PlayerSpritesheet {
    pub image: Handle<Image>,
    pub layout: Handle<TextureAtlasLayout>,
}

const COLUMNS: usize = 14;
const ROWS: usize = 48;
const FRAME_W: u32 = 96;
const FRAME_H: u32 = 84;
const ROW_LAST: [usize; ROWS] = [
    6,7,7,2,0,0,0,7,5,7,7,0,1,1,2,3,5,7,7,5,7,7,7,6,7,7,7,7,7,6,6,6,6,6,6,6,6,6,6,6,6,6,6,6,6,6,6,6
];

/// Default per-frame duration for looping clips (ms).
const DEFAULT_FRAME_MS: u32 = 90;

fn load_player_spritesheet(
    mut atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    assets: Res<AssetServer>,
    mut sheet: ResMut<PlayerSpritesheet>,
) {
    // Load image
    sheet.image = assets.load("PlayerSheet.png");

    // Build an atlas layout that matches the grid in the sheet
    // (Spritesheet::atlas_layout is the convenient way to do this).
    let spritesheet = Spritesheet::new(COLUMNS, ROWS);
    sheet.layout = atlas_layouts.add(spritesheet.atlas_layout(FRAME_W, FRAME_H));
}

fn register_player_animations(
    mut library: ResMut<AnimationLibrary>,
) {
    // Create one animation per row. Name them "player:row00" .. "player:row40".
    // You can then fetch them from any system with:
    //   if let Some(id) = library.animation_with_name("player:row01") { ... }
    let spritesheet = Spritesheet::new(COLUMNS, ROWS);

    for (row, &last_col) in ROW_LAST.iter().enumerate() {
        let frames = if last_col + 1 == COLUMNS {
            spritesheet.row(row)                 // all 8 frames
        } else {
            spritesheet.row_partial(row, 0..=last_col) // only existing frames in that row
        };

        let clip = Clip::from_frames(frames)
            .with_duration(AnimationDuration::PerFrame(DEFAULT_FRAME_MS));

        let clip_id = library.register_clip(clip);
        let anim_id = library.register_animation(Animation::from_clip(clip_id));

        // stable programmatic name
        let _ = library.name_animation(anim_id, &format!("player:row{:02}", row));
    }

    // Optional friendly aliases
    if let Some(id) = library.animation_with_name("player:row00") {
        let _ = library.name_animation(id, "player:idle");
    }
    if let Some(id) = library.animation_with_name("player:row01") {
        let _ = library.name_animation(id, "player:walk");
    }
    if let Some(id) = library.animation_with_name("player:row02") {
        let _ = library.name_animation(id, "player:run");
    }
    if let Some(id) = library.animation_with_name("player:row04") {
        let _ = library.name_animation(id, "player:jump");
    }
    if let Some(id) = library.animation_with_name("player:row43") {
        let _ = library.name_animation(id, "player:attack");
    }
}