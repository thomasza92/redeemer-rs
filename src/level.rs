use crate::prelude::*;
use bevy::ecs::{
        entity::hash_set::EntityHashSet,
        system::{SystemParam, lifetimeless::Read}
    };
use crate::character::Player;
use crate::gameflow::GameplayRoot;

pub fn spawn_map(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    commands
    .spawn((
        TiledMap(asset_server.load("map2.tmx")),
        GameplayRoot,
        Transform::from_xyz(0.0, -100.0, 0.0),
    ))
    .observe(|ev: Trigger<TiledEvent<ColliderCreated>>, mut commands: Commands| {
        commands.entity(ev.event().origin).insert((
            RigidBody::Static,
            Friction::ZERO,
        ));
    });
}


#[derive(Clone, Eq, PartialEq, Debug, Default, Component)]
#[require(ActiveCollisionHooks::MODIFY_CONTACTS)]
pub struct OneWayPlatform(EntityHashSet);

#[derive(Copy, Clone, Eq, PartialEq, Debug, Default, Component, Reflect)]
pub enum PassThroughOneWayPlatform {
    #[default]
    ByNormal,
    Always,
    Never,
}

pub fn pass_through_one_way_platform(
    mut commands: Commands,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut players: Query<(Entity, &mut PassThroughOneWayPlatform), With<Player>>,
) {
    for (entity, mut pass_through_one_way_platform) in &mut players {
        if keyboard_input.pressed(KeyCode::ArrowDown) && keyboard_input.pressed(KeyCode::Space) {
            *pass_through_one_way_platform = PassThroughOneWayPlatform::Always;
            commands.queue(WakeUpBody(entity));
        } else {
            *pass_through_one_way_platform = PassThroughOneWayPlatform::ByNormal;
        }
    }
}

#[derive(SystemParam)]
pub struct PlatformerCollisionHooks<'w, 's> {
    one_way_platforms_query: Query<'w, 's, (Read<OneWayPlatform>, Read<GlobalTransform>)>,
    other_colliders_query: Query<
        'w,
        's,
        Option<Read<PassThroughOneWayPlatform>>,
        (With<Collider>, Without<OneWayPlatform>),
    >,
}

impl CollisionHooks for PlatformerCollisionHooks<'_, '_> {
fn modify_contacts(&self, contacts: &mut ContactPair, commands: &mut Commands) -> bool {
        enum RelevantNormal {
            Normal1,
            Normal2,
        }
        let (platform_entity, one_way_platform, platform_transform, other_entity, relevant_normal) =
            if let Ok((one_way_platform, platform_transform)) =
                self.one_way_platforms_query.get(contacts.collider1)
            {
                (
                    contacts.collider1,
                    one_way_platform,
                    platform_transform,
                    contacts.collider2,
                    RelevantNormal::Normal1,
                )
            } else if let Ok((one_way_platform, platform_transform)) =
                self.one_way_platforms_query.get(contacts.collider2)
            {
                (
                    contacts.collider2,
                    one_way_platform,
                    platform_transform,
                    contacts.collider1,
                    RelevantNormal::Normal2,
                )
            } else {
                return true;
            };

        if one_way_platform.0.contains(&other_entity) {
            let any_penetrating = contacts.manifolds.iter().any(|manifold| {
                manifold
                    .points
                    .iter()
                    .any(|contact| contact.penetration > 0.0)
            });

            if any_penetrating {
                return false;
            } else {
                commands.queue(OneWayPlatformCommand::Remove {
                    platform_entity,
                    entity: other_entity,
                });
            }
        }

        match self.other_colliders_query.get(other_entity) {
            Ok(Some(PassThroughOneWayPlatform::Never)) => true,
            Ok(Some(PassThroughOneWayPlatform::Always)) => {
                commands.queue(OneWayPlatformCommand::Add {
                    platform_entity,
                    entity: other_entity,
                });
                false
            }
            Err(_) | Ok(None) | Ok(Some(PassThroughOneWayPlatform::ByNormal)) => {
                let platform_up = platform_transform.up().truncate().adjust_precision();
                if contacts.manifolds.iter().all(|manifold| {
                    let normal = match relevant_normal {
                        RelevantNormal::Normal1 => manifold.normal,
                        RelevantNormal::Normal2 => -manifold.normal,
                    };

                    normal.length() > Scalar::EPSILON && normal.dot(platform_up) >= 0.5
                }) {
                    true
                } else {
                    commands.queue(OneWayPlatformCommand::Add {
                        platform_entity,
                        entity: other_entity,
                    });
                    false
                }
            }
        }
    }
}

enum OneWayPlatformCommand {
    Add {
        platform_entity: Entity,
        entity: Entity,
    },
    Remove {
        platform_entity: Entity,
        entity: Entity,
    },
}

impl Command for OneWayPlatformCommand {
    fn apply(self, world: &mut World) {
        match self {
            OneWayPlatformCommand::Add {
                platform_entity,
                entity,
            } => {
                if let Some(mut platform) = world.get_mut::<OneWayPlatform>(platform_entity) {
                    platform.0.insert(entity);
                }
            }

            OneWayPlatformCommand::Remove {
                platform_entity,
                entity,
            } => {
                if let Some(mut platform) = world.get_mut::<OneWayPlatform>(platform_entity) {
                    platform.0.remove(&entity);
                }
            }
        }
    }
}