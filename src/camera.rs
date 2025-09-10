use crate::FilmicControls;
use crate::character::Player;
use crate::filmic_post::FilmicSettings;
use crate::halation_post::HalationSettings;
use crate::prelude::*;
use bevy_egui::PrimaryEguiContext;

#[derive(Component)]
pub struct MainCamera;

#[derive(Component)]
pub struct MenuCamera;

pub fn spawn_follow_camera(mut commands: Commands, existing: Query<(), With<MainCamera>>) {
    if existing.is_empty() {
        let mut projection = OrthographicProjection::default_2d();
        projection.scale = 0.33;
        commands.spawn((
            MainCamera,
            Camera2d,
            HalationSettings {
                p0: Vec4::new(0.6, 3.0, 0.7, 0.08),
                p1: Vec4::new(1.0, 0.35, 0.25, 1.25),
                p2: Vec4::new(1.2, 0.0, 0.0, 0.0),
            },
            Msaa::Off,
            FilmicSettings::default(),
            FilmicControls::default(),
            TiledParallaxCamera,
            PrimaryEguiContext,
            Projection::Orthographic(projection),
            Light2d {
                ambient_light: AmbientLight2d {
                    brightness: 0.1,
                    ..default()
                },
            },
        ));
    }
}

pub fn camera_follow(
    time: Res<Time>,
    player_q: Query<&GlobalTransform, With<Player>>,
    mut cam_q: Query<&mut Transform, (With<MainCamera>, Without<Player>)>,
) {
    let Ok(player_gt) = player_q.single() else {
        return;
    };
    let Ok(mut cam_tf) = cam_q.single_mut() else {
        return;
    };
    let cam_adjust = Vec2::new(0., 3.);
    let target_xy = player_gt.translation().truncate() + cam_adjust;
    let current_xy = cam_tf.translation.truncate() + cam_adjust;
    let t = 1.0 - (-10.0 * time.delta_secs()).exp();
    let new_xy = current_xy.lerp(target_xy, t);
    cam_tf.translation.x = new_xy.x;
    cam_tf.translation.y = new_xy.y;
}

pub fn spawn_menu_camera(mut commands: Commands, q_existing: Query<(), With<MenuCamera>>) {
    if q_existing.is_empty() {
        commands.spawn((Camera2d, MenuCamera));
    }
}
pub fn despawn_menu_camera(mut commands: Commands, q: Query<Entity, With<MenuCamera>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}
pub fn despawn_main_camera(mut commands: Commands, q: Query<Entity, With<MainCamera>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}
