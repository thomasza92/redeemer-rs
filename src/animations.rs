use bevy::prelude::*;
use bevy_spritesheet_animation::prelude::*;

pub struct PlayerAnimationsPlugin;

impl Plugin for PlayerAnimationsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerSpritesheet>()
            .add_systems(Startup, (load_player_spritesheet, register_player_animations));
    }
}

#[derive(Resource, Default)]
pub struct PlayerSpritesheet {
    pub image: Handle<Image>,
    pub layout: Handle<TextureAtlasLayout>,
}

const COLUMNS: usize = 14;
const ROWS: usize = 83;
const FRAME_W: u32 = 96;
const FRAME_H: u32 = 84;
const ROW_LAST: [usize; ROWS] = [
    6,7,7,2,0,0,0,0,5,7,7,0,1,1,2,3,5,7,7,5,7,7,7,6,7,7,7,7,7,6,6,6,6,6,6,6,6,6,6,6,6,6,5,5,5,5,6,6,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,5,5,5,5,5,5,5,5,5,5,5,5,5,5,5,5,5
];

const DEFAULT_FRAME_MS: u32 = 100;

fn load_player_spritesheet(
    mut atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    assets: Res<AssetServer>,
    mut sheet: ResMut<PlayerSpritesheet>,
) {
    sheet.image = assets.load("PlayerSheet2.png");

    let spritesheet = Spritesheet::new(COLUMNS, ROWS);
    sheet.layout = atlas_layouts.add(spritesheet.atlas_layout(FRAME_W, FRAME_H));
}

fn register_player_animations(
    mut library: ResMut<AnimationLibrary>,
) {
    let spritesheet = Spritesheet::new(COLUMNS, ROWS);

    for (row, &last_col) in ROW_LAST.iter().enumerate() {
        let frames = if last_col + 1 == COLUMNS {
            spritesheet.row(row)
        } else {
            spritesheet.row_partial(row, 0..=last_col)
        };

        let clip = Clip::from_frames(frames)
            .with_duration(AnimationDuration::PerFrame(DEFAULT_FRAME_MS));

        let clip_id = library.register_clip(clip);
        let anim_id = library.register_animation(Animation::from_clip(clip_id));

        let _ = library.name_animation(anim_id, &format!("player:row{:02}", row));
    }

    if let Some(id) = library.animation_with_name("player:row00") {
        let _ = library.name_animation(id, "player:idle");
    }
    if let Some(id) = library.animation_with_name("player:row01") {
        let _ = library.name_animation(id, "player:walk");
    }
    if let Some(id) = library.animation_with_name("player:row02") {
        let _ = library.name_animation(id, "player:run");
    }
    if let Some(id) = library.animation_with_name("player:row05") {
        let _ = library.name_animation(id, "player:jump");
    }
    if let Some(id) = library.animation_with_name("player:row06") {
        let _ = library.name_animation(id, "player:fall");
    }
    if let Some(id) = library.animation_with_name("player:row77") {
        let _ = library.name_animation(id, "player_combat:idle_attack");
    }
}