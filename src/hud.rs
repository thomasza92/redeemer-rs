use crate::prelude::*;
use bevy::ui::GlobalZIndex;
use crate::class::{PlayerClass, ClassAttachTarget};
use crate::gameflow::GameState;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<PlayerStats>()
            .init_resource::<HudClassSyncState>()
            .add_systems(OnEnter(GameState::InGame), spawn_hud)
            .add_systems(OnExit(GameState::InGame), despawn_hud)
            .add_systems(
                Update,
                (
                    sync_player_stats_from_class,
                    update_health_bar,
                    update_stamina_bar,
                    update_health_text,
                    update_stamina_text,
                )
                .chain()
                .run_if(in_state(GameState::InGame)),
            );
    }
}

#[derive(Component)]
struct HudRoot;

#[derive(Resource)]
pub struct PlayerStats {
    pub health: f32,
    pub max_health: f32,
    pub stamina: f32,
    pub max_stamina: f32,
}
impl Default for PlayerStats {
    fn default() -> Self {
        Self { health: 100.0, max_health: 100.0, stamina: 100.0, max_stamina: 100.0 }
    }
}

#[derive(Resource, Default)]
struct HudClassSyncState {
    last_class_id: Option<String>,
}

#[derive(Component, Default)] struct HealthFill;
#[derive(Component, Default)] struct StaminaFill;
#[derive(Component, Default)] struct HealthText;
#[derive(Component, Default)] struct StaminaText;

fn sync_player_stats_from_class(
    mut stats: ResMut<PlayerStats>,
    mut sync: ResMut<HudClassSyncState>,
    q_class: Query<&PlayerClass, With<ClassAttachTarget>>,
) {
    if let Ok(pc) = q_class.single() {
        let new_id = pc.0.id.clone();
        if sync.last_class_id.as_deref() != Some(&new_id) {
            let b = &pc.0.base_stats;
            stats.max_health   = b.max_health as f32;
            stats.health       = stats.max_health;

            stats.max_stamina  = b.stamina_max;
            stats.stamina      = stats.max_stamina;

            sync.last_class_id = Some(new_id);
        }
    } else {
        sync.last_class_id = None;
    }
}

fn spawn_hud(mut commands: Commands, asset_server: Res<AssetServer>) {
    let root = commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(12.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            ..default()
        },
        GlobalZIndex(1),
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)), // transparent
    )).id();

    let font = asset_server.load("fonts/GohuFont14NerdFontMono-Regular.ttf");

    let hp_row = commands.spawn((
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        },
        BackgroundColor(Color::NONE),
    )).id();

    let hp_label = commands.spawn((
        Text::new("HP"),
        TextFont { font: font.clone(), font_size: 14.0, ..default() },
        TextColor(Color::WHITE),
    )).id();

    let hp_container = commands.spawn((
        Node {
            width: Val::Px(220.0),
            height: Val::Px(18.0),
            border: UiRect::all(Val::Px(2.0)),
            ..default()
        },
        BorderColor(Color::srgb(0.1, 0.1, 0.1)),
        BackgroundColor(Color::srgb(0.05, 0.05, 0.05)),
    )).id();

    let hp_fill = commands.spawn((
        Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
        BackgroundColor(Color::srgb(0.85, 0.2, 0.2)),
        HealthFill,
    )).id();

    let hp_text_overlay = commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            top: Val::Px(0.0),
            bottom: Val::Px(0.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::NONE),
    )).id();

    let hp_text = commands.spawn((
        Text::new("100/100"),
        TextFont { font: font.clone(), font_size: 12.0, ..default() },
        TextColor(Color::WHITE),
        HealthText,
    )).id();

    commands.entity(hp_text_overlay).add_child(hp_text);
    commands.entity(hp_container).add_children(&[hp_fill, hp_text_overlay]);
    commands.entity(hp_row).add_children(&[hp_label, hp_container]);

    let sp_row = commands.spawn((
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        },
        BackgroundColor(Color::NONE),
    )).id();

    let sp_label = commands.spawn((
        Text::new("SP"),
        TextFont { font: font.clone(), font_size: 14.0, ..default() },
        TextColor(Color::WHITE),
    )).id();

    let sp_container = commands.spawn((
        Node {
            width: Val::Px(220.0),
            height: Val::Px(12.0),
            border: UiRect::all(Val::Px(2.0)),
            ..default()
        },
        BorderColor(Color::srgb(0.1, 0.1, 0.1)),
        BackgroundColor(Color::srgb(0.05, 0.05, 0.05)),
    )).id();

    let sp_fill = commands.spawn((
        Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
        BackgroundColor(Color::srgb(0.72, 0.53, 0.04)),
        StaminaFill,
    )).id();

    let sp_text_overlay = commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            top: Val::Px(0.0),
            bottom: Val::Px(0.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::NONE),
    )).id();

    let sp_text = commands.spawn((
        Text::new("100/100"),
        TextFont { font, font_size: 11.0, ..default() },
        TextColor(Color::WHITE),
        StaminaText,
    )).id();

    commands.entity(sp_text_overlay).add_child(sp_text);

    commands.entity(sp_container).add_children(&[sp_fill, sp_text_overlay]);
    commands.entity(sp_row).add_children(&[sp_label, sp_container]);

    commands.entity(root).add_children(&[hp_row, sp_row]);
}

fn update_health_bar(stats: Res<PlayerStats>, mut q: Query<&mut Node, With<HealthFill>>) {
    if let Ok(mut node) = q.single_mut() {
        let pct = (stats.health / stats.max_health).clamp(0.0, 1.0) * 100.0;
        node.width = Val::Percent(pct);
    }
}

fn update_stamina_bar(stats: Res<PlayerStats>, mut q: Query<&mut Node, With<StaminaFill>>) {
    if let Ok(mut node) = q.single_mut() {
        let pct = (stats.stamina / stats.max_stamina).clamp(0.0, 1.0) * 100.0;
        node.width = Val::Percent(pct);
    }
}

fn update_health_text(stats: Res<PlayerStats>, mut q: Query<&mut Text, With<HealthText>>) {
    if let Ok(mut text) = q.single_mut() {
        let cur = stats.health.clamp(0.0, stats.max_health);
        *text = Text::new(format!("{:.0}/{:.0}", cur, stats.max_health));
    }
}

fn update_stamina_text(stats: Res<PlayerStats>, mut q: Query<&mut Text, With<StaminaText>>) {
    if let Ok(mut text) = q.single_mut() {
        let cur = stats.stamina.clamp(0.0, stats.max_stamina);
        *text = Text::new(format!("{:.0}/{:.0}", cur, stats.max_stamina));
    }
}

fn despawn_hud(mut commands: Commands, q: Query<Entity, With<HudRoot>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}