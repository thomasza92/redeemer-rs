use avian2d::{math::*, prelude::*};
use bevy::{
    ecs::{
        entity::hash_set::EntityHashSet,
        system::{SystemParam, lifetimeless::Read},
    },
    prelude::*,
};
use bevy_ecs_tiled::prelude::*;
use bevy::time::Fixed;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                title: String::from("redeemer"),
                        ..Default::default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
                PhysicsPlugins::default()
                // Specify a units-per-meter scaling factor, 1 meter = 20 pixels.
                // The unit allows the engine to tune its parameters for the scale of the world, improving stability.
                .with_length_unit(20.0)
                // Add our custom collision hooks
                .with_collision_hooks::<PlatformerCollisionHooks>(),
        ))
        .add_plugins(TiledPlugin::default())
        .add_plugins(TiledPhysicsPlugin::<TiledPhysicsAvianBackend>::default()) 
        .insert_resource(ClearColor(Color::srgb(0.05, 0.05, 0.1)))
        .insert_resource(Gravity(Vector::NEG_Y * 1000.0))
        .add_systems(Startup, setup)
        .add_systems(FixedUpdate, (dynamic_fall_gravity, movement, pass_through_one_way_platform, camera_follow))
        .run();
}

#[derive(Component)]
struct Actor;

#[derive(Component)]
struct MovementSpeed(Scalar);

#[derive(Component)]
struct JumpImpulse(Scalar);

#[derive(Component)]
struct MainCamera;

#[derive(Component)]
struct DynamicFall {
    base_g: Scalar,   // should match your global Gravity magnitude (1000.0 here)
    max_g:  Scalar,   // terminal gravity “ceiling”
    grow_k: Scalar,   // how quickly we approach max_g (per second)
    t_fall: Scalar,   // internal timer (seconds)
}

// Enable contact modification for one-way platforms with the `ActiveCollisionHooks` component.
// Here we use required components, but you could also add it manually.
#[derive(Clone, Eq, PartialEq, Debug, Default, Component)]
#[require(ActiveCollisionHooks::MODIFY_CONTACTS)]
pub struct OneWayPlatform(EntityHashSet);

/// A component to control how an actor interacts with a one-way platform.
#[derive(Copy, Clone, Eq, PartialEq, Debug, Default, Component, Reflect)]
pub enum PassThroughOneWayPlatform {
    #[default]
    /// Passes through a `OneWayPlatform` if the contact normal is in line with the platform's local-space up vector.
    ByNormal,
    /// Always passes through a `OneWayPlatform`, temporarily set this to allow an actor to jump down through a platform.
    Always,
    /// Never passes through a `OneWayPlatform`.
    Never,
}

fn setup(
    mut commands: Commands,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    asset_server: Res<AssetServer>,
) {
    commands.spawn((Camera2d, MainCamera));
    let map_handle: Handle<TiledMapAsset> = asset_server.load("map.tmx");

    commands
    .spawn((
        TiledMap(map_handle),
        Transform::from_xyz(0.0, -100.0, 0.0),
    ))
    .observe(|ev: Trigger<TiledEvent<ColliderCreated>>, mut commands: Commands| {
        commands.entity(ev.event().origin).insert((
            RigidBody::Static,
            Friction::ZERO,
        ));
    });
    
    let actor_size = Vector::new(20.0, 20.0);
    let actor_mesh = meshes.add(Rectangle::from_size(actor_size.f32()));
    let actor_material = materials.add(Color::srgb(0.2, 0.7, 0.9));

    commands.spawn((
        Mesh2d(actor_mesh),
        MeshMaterial2d(actor_material),
        RigidBody::Dynamic,
        LockedAxes::ROTATION_LOCKED,
        Restitution::ZERO.with_combine_rule(CoefficientCombine::Min),
        Friction::ZERO.with_combine_rule(CoefficientCombine::Min),
        LinearDamping(2.0),
        Collider::circle(actor_size.x - 9.0),
        Actor,
        SpeculativeMargin(0.25),
        PassThroughOneWayPlatform::Never,
        MovementSpeed(250.0),
        JumpImpulse(800.0),
        DynamicFall {
        base_g: 1000.0,
        max_g:  2200.0,
        grow_k: 3.0,
        t_fall: 0.0,
    },
    ));
}

fn camera_follow(
    time: Res<Time>,
    player_q: Query<&GlobalTransform, With<Actor>>,
    // Be explicit to avoid any type shadowing:
    mut cam_q: Query<&mut bevy::prelude::Transform, (With<MainCamera>, Without<Actor>)>,
) {
    let Ok(player_gt) = player_q.single() else { return; };
    let Ok(mut cam_tf)  = cam_q.single_mut() else { return; };

    // Target the player's XY
    let target_xy  = player_gt.translation().truncate();
    let current_xy = cam_tf.translation.truncate();

    // Smooth follow (raise 10.0 to make it snappier)
    let t = 1.0 - (-10.0 * time.delta_secs()).exp();
    let new_xy = current_xy.lerp(target_xy, t);

    // Write just the XY; keep Z as-is
    cam_tf.translation.x = new_xy.x;
    cam_tf.translation.y = new_xy.y;

    // If you want a hard snap instead, do:
    // cam_tf.translation.x = target_xy.x;
    // cam_tf.translation.y = target_xy.y;
}


fn movement(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut actors: Query<(&mut LinearVelocity, &MovementSpeed, &JumpImpulse), With<Actor>>,
) {
    for (mut linear_velocity, movement_speed, jump_impulse) in &mut actors {
        // Naive grounded check (matches your jump logic)
        let grounded = linear_velocity.y.abs() < 0.1;

        // Input
        let left  = keyboard_input.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]);
        let right = keyboard_input.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]);
        let horizontal = right as i8 - left as i8;

        // Sprint (Shift)
        let sprinting = keyboard_input.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
        let sprint_mul: Scalar = if sprinting { 1.6 } else { 1.0 };

        if grounded {
            // On ground: allow horizontal input (with sprint)
            linear_velocity.x = horizontal as Scalar * movement_speed.0 * sprint_mul;
        } else {
            // In air: block horizontal input
            // OPTION A (keep pre-jump momentum): do nothing
            // OPTION B (freeze X completely while airborne): uncomment the next line
            // linear_velocity.x = 0.0;
        }

        // Jump only when grounded (your existing logic)
        if grounded
            && !keyboard_input.pressed(KeyCode::ArrowDown)
            && keyboard_input.just_pressed(KeyCode::Space)
        {
            linear_velocity.y = jump_impulse.0;
        }
    }
}


fn dynamic_fall_gravity(
    time: Res<Time<Fixed>>,
    mut q: Query<(&mut LinearVelocity, &mut DynamicFall), With<Actor>>,
) {
    let dt = time.delta_secs();
    for (mut vel, mut dynfall) in &mut q {
        if vel.y < 0.0 {
            // accumulate time while falling
            dynfall.t_fall += dt;

            // g(t) = base + (max-base)*(1 - e^{-k t})
            let g_t = dynfall.base_g
                + (dynfall.max_g - dynfall.base_g) * (1.0 - (-dynfall.grow_k * dynfall.t_fall).exp());

            // apply only the *extra* beyond base_g (global Gravity already applies base_g)
            let extra = g_t - dynfall.base_g;
            vel.y -= extra * dt; // negative is downward
        } else {
            // not falling: reset timer so next fall starts fresh
            dynfall.t_fall = 0.0;
        }
    }
}


fn pass_through_one_way_platform(
    mut commands: Commands,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut actors: Query<(Entity, &mut PassThroughOneWayPlatform), With<Actor>>,
) {
    for (entity, mut pass_through_one_way_platform) in &mut actors {
        if keyboard_input.pressed(KeyCode::ArrowDown) && keyboard_input.pressed(KeyCode::Space) {
            *pass_through_one_way_platform = PassThroughOneWayPlatform::Always;

            // Wake up the body when it's allowed to drop down.
            // Otherwise it won't fall because gravity isn't simulated.
            commands.queue(WakeUpBody(entity));
        } else {
            *pass_through_one_way_platform = PassThroughOneWayPlatform::ByNormal;
        }
    }
}

// Define a custom `SystemParam` for our collision hooks.
// It can have read-only access to queries, resources, and other system parameters.
#[derive(SystemParam)]
struct PlatformerCollisionHooks<'w, 's> {
    one_way_platforms_query: Query<'w, 's, (Read<OneWayPlatform>, Read<GlobalTransform>)>,
    // NOTE: This precludes a `OneWayPlatform` passing through a `OneWayPlatform`.
    other_colliders_query: Query<
        'w,
        's,
        Option<Read<PassThroughOneWayPlatform>>,
        (With<Collider>, Without<OneWayPlatform>),
    >,
}

// Implement the `CollisionHooks` trait for our custom system parameter.
impl CollisionHooks for PlatformerCollisionHooks<'_, '_> {
    // Below is a description of the logic used for one-way platforms.

    /// Allows entities to pass through [`OneWayPlatform`] entities.
    ///
    /// Passing through is achieved by removing the collisions between the [`OneWayPlatform`]
    /// and the other entity if the entity should pass through.
    /// If a [`PassThroughOneWayPlatform`] is present on the non-platform entity,
    /// the value of the component dictates the pass-through behaviour.
    ///
    /// Entities known to be passing through each [`OneWayPlatform`] are stored in the
    /// [`OneWayPlatform`]. If an entity is known to be passing through a [`OneWayPlatform`],
    /// it is allowed to continue to do so, even if [`PassThroughOneWayPlatform`] has been
    /// set to disallow passing through.
    ///
    /// #### When an entity is known to already be passing through the [`OneWayPlatform`]
    ///
    /// When an entity begins passing through a [`OneWayPlatform`], it is added to the
    /// [`OneWayPlatform`]'s set of active penetrations, and will be allowed to continue
    /// to pass through until it is no longer penetrating the platform.
    ///
    /// #### When an entity is *not* known to be passing through the [`OneWayPlatform`]
    ///
    /// Depending on the setting of [`PassThroughOneWayPlatform`], the entity may be allowed to
    /// pass through.
    ///
    /// If no [`PassThroughOneWayPlatform`] is present, [`PassThroughOneWayPlatform::ByNormal`] is used.
    ///
    /// [`PassThroughOneWayPlatform`] may be in one of three states:
    /// 1. [`PassThroughOneWayPlatform::ByNormal`]
    ///     - This is the default state
    ///     - The entity may be allowed to pass through the [`OneWayPlatform`] depending on the contact normal
    ///         - If all contact normals are in line with the [`OneWayPlatform`]'s local-space up vector,
    ///           the entity is allowed to pass through
    /// 2. [`PassThroughOneWayPlatform::Always`]
    ///     - The entity will always pass through the [`OneWayPlatform`], regardless of contact normal
    ///     - This is useful for allowing an entity to jump down through a platform
    /// 3. [`PassThroughOneWayPlatform::Never`]
    ///     - The entity will never pass through the [`OneWayPlatform`], meaning the platform will act
    ///       as normal hard collision for this entity
    ///
    /// Even if an entity is changed to [`PassThroughOneWayPlatform::Never`], it will be allowed to pass
    /// through a [`OneWayPlatform`] if it is already penetrating the platform. Once it exits the platform,
    /// it will no longer be allowed to pass through.
    fn modify_contacts(&self, contacts: &mut ContactPair, commands: &mut Commands) -> bool {
        // This is the contact modification hook, called after collision detection,
        // but before constraints are created for the solver. Mutable access to the ECS
        // is not allowed, but we can queue commands to perform deferred changes.

        // Differentiate between which normal of the manifold we should use
        enum RelevantNormal {
            Normal1,
            Normal2,
        }

        // First, figure out which entity is the one-way platform, and which is the other.
        // Choose the appropriate normal for pass-through depending on which is which.
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
                // Neither is a one-way-platform, so accept the collision:
                // we're done here.
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
                // If we were already allowing a collision for a particular entity,
                // and if it is penetrating us still, continue to allow it to do so.
                return false;
            } else {
                // If it's no longer penetrating us, forget it.
                commands.queue(OneWayPlatformCommand::Remove {
                    platform_entity,
                    entity: other_entity,
                });
            }
        }

        match self.other_colliders_query.get(other_entity) {
            // Pass-through is set to never, so accept the collision.
            Ok(Some(PassThroughOneWayPlatform::Never)) => true,
            // Pass-through is set to always, so always ignore this collision
            // and register it as an entity that's currently penetrating.
            Ok(Some(PassThroughOneWayPlatform::Always)) => {
                commands.queue(OneWayPlatformCommand::Add {
                    platform_entity,
                    entity: other_entity,
                });
                false
            }
            // Default behaviour is "by normal".
            Err(_) | Ok(None) | Ok(Some(PassThroughOneWayPlatform::ByNormal)) => {
                // If all contact normals are in line with the local up vector of this platform,
                // then this collision should occur: the entity is on top of the platform.
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
                    // Otherwise, ignore the collision and register
                    // the other entity as one that's currently penetrating.
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

/// A command to add/remove entities to/from the set of entities
/// that are currently in contact with a one-way platform.
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