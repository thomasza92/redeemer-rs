// enemy_spawner.rs
use bevy::math::Dir2;
use bevy::prelude::*;
use bevy_ecs_tiled::prelude::*;
use rand::{Rng, rng};

use avian2d::collision::collider::LayerMask;
use avian2d::spatial_query::{SpatialQuery, SpatialQueryFilter};

use crate::character::GameLayer; // your PhysicsLayer enum from character.rs
use crate::enemy::spawn_enemy; // your existing enemy spawner function

/// Configuration + timer for periodic enemy spawns.
#[derive(Resource)]
pub struct EnemySpawner {
    pub timer: Timer,
    pub attempts_per_tick: u32,
    pub ray_down: f32,
    pub y_above_ground: f32,
    pub _patrol_span: f32,
    pub spawn_z: f32, // ← add this
}

impl Default for EnemySpawner {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(5.0, TimerMode::Repeating),
            attempts_per_tick: 8,
            ray_down: 2000.0,
            y_above_ground: 8.0,
            _patrol_span: 100.0,
            spawn_z: -100.1,
        }
    }
}

/// Convert the tilemap components into a world-space AABB (bottom-left, top-right).
/// Assumes no rotation/scaling on the tilemap transform (standard setup).
fn tilemap_world_aabb(
    size: &TilemapSize,
    grid: &TilemapGridSize,
    tile: &TilemapTileSize,
    ty: &TilemapType,
    anchor: TilemapAnchor,
    xform: &GlobalTransform,
) -> (Vec2, Vec2) {
    // Anchor offset tells where the *map’s* bottom-left corner is relative to the transform.
    let offset = anchor.as_offset(size, grid, tile, ty);
    let origin = xform.translation().truncate() + offset;

    let w = size.x as f32 * grid.x;
    let h = size.y as f32 * grid.y;

    let min = origin;
    let max = origin + Vec2::new(w, h);
    (min, max)
}

/// Pick a random X within the tilemap’s horizontal span.
fn random_x_in_map(min: Vec2, max: Vec2) -> f32 {
    let mut r = rng();
    r.random_range(min.x..max.x)
}

/// Try to find a valid spawn point: pick a random X, raycast downward to ground,
/// and return the position slightly above the hit point, plus patrol bounds.
fn try_pick_spawn_point(
    min: Vec2,
    max: Vec2,
    spatial: &SpatialQuery,
    y_above: f32,
    ray_down: f32,
) -> Option<(Vec2, f32, f32)> {
    let x = random_x_in_map(min, max);

    // Start well above the map’s top edge so we always cast through empty space first.
    let start = Vec2::new(x, max.y + 200.0);

    // Avian 2D: cast_ray(origin, direction: Dir2, max_distance, solid, filter)
    let dir = Dir2::from_xy(0.0, -1.0).unwrap();

    // Only consider ground/default layer as valid “floor”.
    let filter = SpatialQueryFilter::from_mask(LayerMask::from(GameLayer::Default));
    if let Some(hit) = spatial.cast_ray(start, dir, ray_down, true, &filter) {
        // Reconstruct the hit point from distance along ray.
        let hit_point = start + dir.as_vec2() * hit.distance;
        let spawn_pos = Vec2::new(x, hit_point.y + y_above);
        // Small patrol around the spawn X
        let patrol_left = x - 100.0;
        let patrol_right = x + 100.0;
        Some((spawn_pos, patrol_left, patrol_right))
    } else {
        None
    }
}

/// System: tick the spawn timer and spawn when it elapses.
fn tick_enemy_spawner(
    time: Res<Time>,
    mut spawner: ResMut<EnemySpawner>,
    // Grab *any* tile layer to derive map bounds (all layers share size/grid/anchor).
    map_q: Query<(
        &TilemapSize,
        &TilemapGridSize,
        &TilemapTileSize,
        &TilemapType,
        Option<&TilemapAnchor>,
        &GlobalTransform,
    )>,
    spatial: SpatialQuery, // NOTE: this is a system parameter, NOT `Res<_>`
    mut commands: Commands,
) {
    if map_q.is_empty() {
        return;
    }

    spawner.timer.tick(time.delta());
    if !spawner.timer.just_finished() {
        return;
    }

    // Use the first map layer we find to compute world bounds.
    let (size, grid, tile, ty, maybe_anchor, gt) = match map_q.iter().next() {
        Some(t) => t,
        None => return,
    };
    let anchor = maybe_anchor.copied().unwrap_or(TilemapAnchor::BottomLeft);
    let (min, max) = tilemap_world_aabb(size, grid, tile, ty, anchor, gt);

    for _ in 0..spawner.attempts_per_tick {
        if let Some((pos, left, right)) =
            try_pick_spawn_point(min, max, &spatial, spawner.y_above_ground, spawner.ray_down)
        {
            let e = spawn_enemy(&mut commands, pos, left, right);
            commands
                .entity(e)
                .insert(Transform::from_xyz(pos.x, pos.y, spawner.spawn_z));
            break;
        }
    }
}

/// Tiny plugin to wire everything up.
pub struct EnemySpawnerPlugin;

impl Plugin for EnemySpawnerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EnemySpawner>()
            .add_systems(Update, tick_enemy_spawner);
    }
}
