use crate::class::{ClassAttachTarget, PlayerClass};
use crate::hud::PlayerStats;
use avian2d::spatial_query::{RayCaster, RayHits, SpatialQueryFilter};
use bevy::prelude::*;
use bevy::sprite::Sprite;
use std::collections::HashSet;

#[derive(Component, Clone)]
pub struct MeleeRaycastSpec {
    pub offset: Vec2,
    pub length: f32,
    pub max_hits: u32,
    pub damage: i32,
    pub filter: SpatialQueryFilter,
    pub solid: bool,
    pub once_per_swing: bool,
}

#[derive(Component, Default)]
pub struct MeleeAttackActive;

#[derive(Event, Debug, Clone)]
pub struct MeleeRaycastHit {
    pub attacker: Entity,
    pub target: Entity,
    pub distance: f32,
    pub normal: Vec2,
    pub damage: i32,
}

#[derive(Component)]
struct AttackRay;

#[derive(Component, Default)]
struct AlreadyHit(HashSet<Entity>);

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub enum RaycastMeleeSet {
    Cast,
    ApplyDamage,
}

pub struct RaycastMeleePlugin;

impl Plugin for RaycastMeleePlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<MeleeRaycastHit>()
            .configure_sets(
                Update,
                (RaycastMeleeSet::Cast, RaycastMeleeSet::ApplyDamage).chain(),
            )
            .add_systems(
                Update,
                (
                    spawn_ray_on_attack_start,
                    keep_ray_facing_correctly,
                    emit_hits_from_rays,
                )
                    .in_set(RaycastMeleeSet::Cast),
            )
            .add_systems(
                Update,
                apply_melee_damage_to_player_stats.in_set(RaycastMeleeSet::ApplyDamage),
            )
            .add_systems(Update, despawn_ray_on_attack_end);
    }
}

fn is_facing_right(sprite: Option<&Sprite>, gt: Option<&GlobalTransform>) -> bool {
    if let Some(s) = sprite {
        return !s.flip_x;
    }
    if let Some(t) = gt {
        return t.scale().x >= 0.0;
    }
    true
}

fn spawn_ray_on_attack_start(
    mut commands: Commands,
    added: Query<(Entity, &MeleeRaycastSpec), Added<MeleeAttackActive>>,
    sprites: Query<&Sprite>,
    globals: Query<&GlobalTransform>,
) {
    for (attacker, spec) in &added {
        commands.entity(attacker).insert(AlreadyHit::default());

        let sprite = sprites.get(attacker).ok();
        let gt = globals.get(attacker).ok();
        let facing_right = is_facing_right(sprite, gt);

        let origin = if facing_right {
            spec.offset
        } else {
            Vec2::new(-spec.offset.x, spec.offset.y)
        };
        let direction = if facing_right { Dir2::X } else { Dir2::NEG_X };

        commands.entity(attacker).with_children(|c| {
            c.spawn((
                AttackRay,
                Transform::default(),
                GlobalTransform::default(),
                RayCaster::new(origin, direction)
                    .with_max_distance(spec.length)
                    .with_max_hits(spec.max_hits)
                    .with_ignore_self(true)
                    .with_solidness(spec.solid)
                    .with_query_filter(spec.filter.clone()),
            ));
        });
    }
}

fn keep_ray_facing_correctly(
    attackers: Query<(Entity, Option<&Sprite>, Option<&GlobalTransform>), With<MeleeAttackActive>>,
    children: Query<&Children>,
    mut rays: Query<&mut RayCaster, With<AttackRay>>,
    specs: Query<&MeleeRaycastSpec>,
) {
    for (attacker, sprite, gt) in &attackers {
        let facing_right = is_facing_right(sprite, gt);
        let Ok(spec) = specs.get(attacker) else {
            continue;
        };

        let origin = if facing_right {
            spec.offset
        } else {
            Vec2::new(-spec.offset.x, spec.offset.y)
        };
        let dir = if facing_right { Dir2::X } else { Dir2::NEG_X };

        if let Ok(kids) = children.get(attacker) {
            for &child in kids {
                if let Ok(mut rc) = rays.get_mut(child) {
                    rc.origin = origin;
                    rc.direction = dir;
                }
            }
        }
    }
}

fn apply_melee_damage_to_player_stats(
    mut events: EventReader<MeleeRaycastHit>,
    mut stats: ResMut<PlayerStats>,
    targets_with_player_tag: Query<Entity, With<ClassAttachTarget>>,
    defenses: Query<&PlayerClass>,
) {
    for hit in events.read() {
        if targets_with_player_tag.get(hit.target).is_ok() {
            let defense = defenses
                .get(hit.target)
                .map(|pc| pc.0.base_stats.defense)
                .unwrap_or(0.0)
                .clamp(0.0, 0.95);

            let reduced = (hit.damage as f32) * (1.0 - defense);
            let dmg = reduced.max(0.0).ceil();

            stats.health = (stats.health - dmg).max(0.0);
        }
    }
}

fn emit_hits_from_rays(
    mut writer: EventWriter<MeleeRaycastHit>,
    rays: Query<(&ChildOf, &RayHits), With<AttackRay>>,
    specs: Query<&MeleeRaycastSpec>,
    mut hit_sets: Query<&mut AlreadyHit>,
) {
    for (child_of, ray_hits) in &rays {
        let attacker = child_of.0; // parent entity
        let Ok(spec) = specs.get(attacker) else {
            continue;
        };

        for hit in ray_hits.iter_sorted() {
            let target = hit.entity;

            if spec.once_per_swing {
                if let Ok(mut set) = hit_sets.get_mut(attacker) {
                    if set.0.contains(&target) {
                        continue;
                    }
                    set.0.insert(target);
                }
            }

            writer.write(MeleeRaycastHit {
                attacker,
                target,
                distance: hit.distance,
                normal: hit.normal,
                damage: spec.damage,
            });
        }
    }
}

fn despawn_ray_on_attack_end(
    mut commands: Commands,
    mut removed: RemovedComponents<MeleeAttackActive>,
    children: Query<&Children>,
    rays: Query<Entity, With<AttackRay>>,
) {
    for attacker in removed.read() {
        if let Ok(kids) = children.get(attacker) {
            for &child in kids {
                if rays.get(child).is_ok() {
                    // Bevy 0.16: recursive by default
                    commands.entity(child).despawn();
                }
            }
        }
    }
}
