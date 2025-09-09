// enemy.rs
use avian2d::collision::collider::{CollisionLayers, LayerMask};
use avian2d::prelude::*;
use avian2d::spatial_query::SpatialQueryFilter;
use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy_spritesheet_animation::prelude::*;
use big_brain::prelude::*;

use crate::character::{GameLayer, Player};
use crate::raycasts::{MeleeAttackActive, MeleeRaycastSpec};

// ====== Animation bits ======
#[derive(Component, Clone, Copy)]
struct EnemyCurrentAnim(AnimationId);

#[derive(Component, Clone, Copy)]
pub struct EnemyAnimClips {
    pub idle: AnimationId,
    pub walk: Option<AnimationId>,
    pub run: Option<AnimationId>,
    pub jump: Option<AnimationId>,
    pub fall: Option<AnimationId>,
    pub attack_idle: AnimationId,
    pub attack_walk: Option<AnimationId>,
    pub attack_run: Option<AnimationId>,
    pub attack_jump: Option<AnimationId>,
    pub attack_fall: Option<AnimationId>,
}

// ====== Tags & data ======
#[derive(Component)]
pub struct Enemy;

#[derive(Component, Clone, Copy)]
pub struct PatrolBounds {
    pub left: f32,
    pub right: f32,
}

#[derive(Component, Deref, DerefMut)]
pub struct PatrolDir(pub f32);

#[derive(Component, Default, Debug, Clone, Copy)]
pub struct EnemySenses {
    pub target: Option<Entity>,
    pub target_pos: Vec2,
    pub dx: f32,
    pub dist: f32,
}

#[derive(Component)]
struct EnemyAttackTimer(Timer);
#[derive(Component)]
struct EnemyAttackCooldown(Timer);

// ====== Tuning ======
const WALK: f32 = 50.0;
const RUN: f32 = 200.0;
const ACCEL: f32 = 3000.0;
const AGGRO: f32 = 260.0;
const RANGE: f32 = 46.0;
const SWING: f32 = 0.35;
const COOLDOWN: f32 = 0.60;

// ====== Bundle (avoid gigantic tuple bundles) ======
#[derive(Bundle)]
pub struct EnemyBundle {
    enemy: Enemy,
    patrol: PatrolBounds,
    dir: PatrolDir,
    senses: EnemySenses,

    // physics
    body: RigidBody,
    lock: LockedAxes,
    restitution: Restitution,
    friction: Friction,
    damping: LinearDamping,
    collider: Collider,
    speculative: SpeculativeMargin,
    collisions: CollidingEntities,
    transform: Transform,
    global_transform: GlobalTransform,
    vel: LinearVelocity,
    layers: CollisionLayers,

    // melee config
    ray: MeleeRaycastSpec,

    name: Name,
}

pub fn spawn_enemy(cmd: &mut Commands, pos: Vec2, left: f32, right: f32) -> Entity {
    let player_mask = SpatialQueryFilter::from_mask(LayerMask::from(GameLayer::Player));

    cmd.spawn(EnemyBundle {
        enemy: Enemy,
        patrol: PatrolBounds { left, right },
        dir: PatrolDir(1.0),
        senses: EnemySenses::default(),

        body: RigidBody::Dynamic,
        lock: LockedAxes::ROTATION_LOCKED,
        restitution: Restitution::ZERO.with_combine_rule(CoefficientCombine::Min),
        friction: Friction::ZERO.with_combine_rule(CoefficientCombine::Min),
        damping: LinearDamping(2.0),
        collider: Collider::capsule(8.0, 26.0),
        speculative: SpeculativeMargin(0.1),
        collisions: CollidingEntities::default(),
        transform: Transform::from_xyz(pos.x, pos.y, -1.0),
        global_transform: GlobalTransform::default(),
        vel: LinearVelocity::default(),
        layers: CollisionLayers::new(
            LayerMask::from(GameLayer::Enemy),
            LayerMask::from(GameLayer::Player) | LayerMask::from(GameLayer::Default),
        ),
        ray: MeleeRaycastSpec {
            offset: Vec2::new(16.0, 8.0),
            length: RANGE,
            max_hits: 1,
            damage: 20,
            filter: player_mask,
            solid: false,
            once_per_swing: true,
        },
        name: Name::new("Enemy"),
    })
    .insert(
        Thinker::build()
            .picker(FirstToScore::new(0.5))
            .when(AttackInRange, Attack) // in range -> Attack
            .when(HasTarget, Chase) // aggro only -> Chase
            .otherwise(Patrol), // no target -> Patrol
    )
    .id()
}

// ====== Scorers ======
#[derive(Debug, Clone, Component, ScorerBuilder)]
pub struct HasTarget;

fn has_target_scorer(
    mut q: Query<(&Actor, &mut Score), With<HasTarget>>,
    senses: Query<&EnemySenses>,
) {
    for (Actor(actor), mut score) in q.iter_mut() {
        let has = senses.get(*actor).ok().and_then(|s| s.target).is_some();
        score.set(if has { 1.0 } else { 0.0 });
    }
}

#[derive(Debug, Clone, Component, ScorerBuilder)]
pub struct AttackInRange;

/// Minimal, robust scorer:
/// - 1.0 while swinging (prevents preemption)
/// - 0.0 on cooldown
/// - Otherwise: choose Attack when horizontally close to the target
fn attack_in_range_scorer(
    mut q: Query<(&Actor, &mut Score), With<AttackInRange>>,
    senses: Query<&EnemySenses>,
    swinging_q: Query<Option<&MeleeAttackActive>>,
    cd_q: Query<Option<&EnemyAttackCooldown>>,
) {
    const ATTACK_BAND_X: f32 = RANGE + 24.0;

    for (Actor(actor), mut score) in q.iter_mut() {
        if swinging_q.get(*actor).ok().flatten().is_some() {
            score.set(1.0);
            continue;
        }
        let on_cd = cd_q
            .get(*actor)
            .ok()
            .flatten()
            .map(|c| !c.0.finished())
            .unwrap_or(false);
        if on_cd {
            score.set(0.0);
            continue;
        }

        let ok = senses
            .get(*actor)
            .ok()
            .map(|s| s.target.is_some() && s.dx.abs() <= ATTACK_BAND_X)
            .unwrap_or(false);

        score.set(if ok { 1.0 } else { 0.0 });
    }
}

// ====== Actions ======
#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct Patrol;

fn patrol_action(
    time: Res<Time>,
    mut q: Query<(&Actor, &mut ActionState), With<Patrol>>,
    mut movers: Query<(
        &mut LinearVelocity,
        &GlobalTransform,
        &mut PatrolDir,
        &PatrolBounds,
    )>,
) {
    for (Actor(actor), mut state) in q.iter_mut() {
        match *state {
            ActionState::Init | ActionState::Requested => {
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                if let Ok((mut vel, gt, mut dir, bounds)) = movers.get_mut(*actor) {
                    let x = gt.translation().x;
                    if x <= bounds.left {
                        dir.0 = 1.0;
                    }
                    if x >= bounds.right {
                        dir.0 = -1.0;
                    }

                    let target_vx = dir.0 * WALK;
                    let accel = ACCEL * time.delta_secs();
                    let delta = (target_vx - vel.x).clamp(-accel, accel);
                    vel.x += delta;
                }
            }
            ActionState::Cancelled => {
                *state = ActionState::Failure; // allow preemption
            }
            ActionState::Success | ActionState::Failure => {
                *state = ActionState::Requested;
            }
        }
    }
}

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct Chase;

fn chase_action(
    time: Res<Time>,
    mut q: Query<(&Actor, &mut ActionState), With<Chase>>,
    mut movers: Query<(&mut LinearVelocity, &GlobalTransform)>,
    senses: Query<&EnemySenses>,
) {
    for (Actor(actor), mut state) in q.iter_mut() {
        match *state {
            ActionState::Init | ActionState::Requested => {
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                if let (Ok((mut vel, gt)), Ok(s)) = (movers.get_mut(*actor), senses.get(*actor)) {
                    if let Some(_t) = s.target {
                        let dx = s.target_pos.x - gt.translation().x;
                        let dir = dx.signum();

                        // Slow/stop just inside attack band so Attack scorer can take over
                        let desired = if s.dist <= RANGE + 8.0 {
                            0.0
                        } else {
                            dir * RUN
                        };
                        let accel = ACCEL * time.delta_secs();
                        let delta = (desired - vel.x).clamp(-accel, accel);
                        vel.x += delta;
                    } else {
                        *state = ActionState::Success;
                    }
                } else {
                    *state = ActionState::Failure;
                }
            }
            ActionState::Cancelled => {
                *state = ActionState::Failure; // allow preemption
            }
            ActionState::Success | ActionState::Failure => {
                *state = ActionState::Requested;
            }
        }
    }
}

#[derive(Debug, Clone, Component, ActionBuilder)]
pub struct Attack;

fn attack_action(
    mut cmd: Commands,
    mut q: Query<(&Actor, &mut ActionState), With<Attack>>,
    mut timers: Query<(
        Option<&mut EnemyAttackTimer>,
        Option<&mut EnemyAttackCooldown>,
    )>,
    mut vels: Query<&mut LinearVelocity>,
) {
    for (Actor(actor), mut state) in q.iter_mut() {
        match *state {
            ActionState::Init | ActionState::Requested => {
                // Do not start if on cooldown or already swinging
                let (swinging, on_cd) = if let Ok((maybe_timer, maybe_cd)) = timers.get_mut(*actor)
                {
                    let swinging = maybe_timer.is_some();
                    let on_cd = maybe_cd.as_ref().map(|c| !c.0.finished()).unwrap_or(false);
                    (swinging, on_cd)
                } else {
                    (false, false)
                };

                if !on_cd && !swinging {
                    cmd.entity(*actor).insert((
                        MeleeAttackActive,
                        EnemyAttackTimer(Timer::from_seconds(SWING, TimerMode::Once)),
                    ));
                    if let Ok(mut v) = vels.get_mut(*actor) {
                        v.x = 0.0;
                    }
                    *state = ActionState::Executing;
                } else {
                    *state = ActionState::Failure;
                }
            }

            ActionState::Executing => {
                // Hold still while the swing timer runs
                if let Ok(mut v) = vels.get_mut(*actor) {
                    v.x = 0.0;
                }

                if let Ok((maybe_timer, _)) = timers.get_mut(*actor) {
                    let done = maybe_timer
                        .as_ref()
                        .map(|t| t.0.finished())
                        .unwrap_or(false);
                    if done {
                        // Swing finished: end swing and NOW start cooldown.
                        cmd.entity(*actor)
                            .remove::<MeleeAttackActive>()
                            .remove::<EnemyAttackTimer>()
                            .insert(EnemyAttackCooldown(Timer::from_seconds(
                                COOLDOWN,
                                TimerMode::Once,
                            )));
                        *state = ActionState::Success;
                    }
                } else {
                    // If we somehow lost the timer, bail without triggering cooldown.
                    cmd.entity(*actor).remove::<MeleeAttackActive>();
                    *state = ActionState::Failure;
                }
            }

            ActionState::Cancelled => {
                // Cancel means “didn’t complete swing”; no cooldown here.
                cmd.entity(*actor)
                    .remove::<MeleeAttackActive>()
                    .remove::<EnemyAttackTimer>();
                *state = ActionState::Failure;
            }

            ActionState::Success | ActionState::Failure => {
                *state = ActionState::Requested;
            }
        }
    }
}

// ====== Perception & misc ======
fn sense_player(
    players: Query<(Entity, &GlobalTransform), With<Player>>,
    mut enemies: Query<(&GlobalTransform, &mut EnemySenses), With<Enemy>>,
) {
    // Choose the first (or only) player
    let player = players.iter().next();
    if let Some((pe, pgt)) = player {
        let p = pgt.translation().truncate();
        for (egt, mut s) in enemies.iter_mut() {
            let e = egt.translation().truncate();
            s.target = if p.distance(e) <= AGGRO {
                Some(pe)
            } else {
                None
            };
            s.target_pos = p;
            s.dx = p.x - e.x;
            s.dist = p.distance(e);
        }
    } else {
        for (_egt, mut s) in enemies.iter_mut() {
            s.target = None;
        }
    }
}

/// Always face the target if aggro’d; fallback to velocity otherwise.
/// Sprite.flip_x is what your raycasts use to aim the ray.
fn face_by_target_or_velocity(
    mut q: Query<(&mut Sprite, &LinearVelocity, &EnemySenses), With<Enemy>>,
) {
    for (mut sprite, vel, senses) in q.iter_mut() {
        let dir = if senses.target.is_some() {
            senses.dx.signum()
        } else {
            vel.x.signum()
        };
        if dir > 0.05 {
            sprite.flip_x = false; // facing right
        } else if dir < -0.05 {
            sprite.flip_x = true; // facing left
        }
    }
}

fn on_enemy_added_attach_sprite_and_anims(
    mut commands: Commands,
    sheet: Res<crate::animations::PlayerSpritesheet>, // reuse your existing spritesheet asset
    library: Res<AnimationLibrary>,
    added: Query<Entity, Added<Enemy>>,
) {
    for e in &added {
        // If you have an "enemy_combat:..." set, swap names accordingly.
        let idle_id = library
            .animation_with_name("player_combat:swordidle")
            .expect("missing animation: player_combat:swordidle");

        let clips = EnemyAnimClips {
            idle: idle_id,
            walk: library.animation_with_name("player_combat:swordrun"),
            run: library.animation_with_name("player_combat:swordsprint"),
            jump: library.animation_with_name("player_combat:swordjumpmid"),
            fall: library.animation_with_name("player_combat:swordjumpfall"),
            attack_idle: library
                .animation_with_name("player_combat:standingslash")
                .expect("missing animation: player_combat:standingslash"),
            attack_walk: library.animation_with_name("player_combat:swordrunslash"),
            attack_run: library.animation_with_name("player_combat:swordsprintslash"),
            attack_jump: library.animation_with_name("player_combat:airslashup"),
            attack_fall: library.animation_with_name("player_combat:airslashdown"),
        };

        let mut sprite = Sprite::from_atlas_image(
            sheet.image.clone(),
            TextureAtlas {
                layout: sheet.layout.clone(),
                ..Default::default()
            },
        );
        sprite.anchor = Anchor::Custom(Vec2::new(0.0, -0.3));

        let mut anim = SpritesheetAnimation::from_id(idle_id);
        anim.playing = true;

        commands
            .entity(e)
            .insert((sprite, anim, clips, EnemyCurrentAnim(idle_id)));
    }
}

fn drive_enemy_animation(
    mut q: Query<
        (
            &EnemyAnimClips,
            &mut SpritesheetAnimation,
            &mut EnemyCurrentAnim,
            &LinearVelocity,
            Option<&CollidingEntities>,
            Option<&MeleeAttackActive>,
        ),
        With<Enemy>,
    >,
) {
    for (clips, mut anim, mut current, vel, contacts, melee) in &mut q {
        let on_ground = contacts.map(|c| !c.is_empty()).unwrap_or(true);
        let in_air = !on_ground;
        let speed = vel.x.abs();
        let moving = speed > 6.0;
        let running = speed > (RUN * 0.7);

        let want = if melee.is_some() {
            if in_air {
                if vel.y <= 0.0 {
                    clips
                        .attack_fall
                        .or(clips.attack_jump)
                        .or(Some(clips.attack_idle))
                } else {
                    clips
                        .attack_jump
                        .or(clips.attack_fall)
                        .or(Some(clips.attack_idle))
                }
            } else if running {
                clips
                    .attack_run
                    .or(clips.attack_walk)
                    .or(Some(clips.attack_idle))
            } else if moving {
                clips.attack_walk.or(Some(clips.attack_idle))
            } else {
                Some(clips.attack_idle)
            }
        } else if in_air {
            if vel.y > 0.0 {
                clips.jump.or(clips.fall)
            } else {
                clips.fall
            }
            .or(Some(clips.idle))
        } else if running {
            clips.run.or(clips.walk).or(Some(clips.idle))
        } else if moving {
            clips.walk.or(Some(clips.idle))
        } else {
            Some(clips.idle)
        };

        if let Some(id) = want {
            if current.0 != id {
                *anim = SpritesheetAnimation::from_id(id);
                anim.playing = true;
                current.0 = id;
            }
        }
    }
}

fn tick_enemy_attack_timers(
    time: Res<Time>,
    mut atk: Query<&mut EnemyAttackTimer>,
    mut cds: Query<&mut EnemyAttackCooldown>,
) {
    for mut t in atk.iter_mut() {
        t.0.tick(time.delta());
    }
    for mut c in cds.iter_mut() {
        c.0.tick(time.delta());
    }
}

// ====== Plugin wiring ======
pub struct EnemyPlugin;

impl Plugin for EnemyPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(BigBrainPlugin::new(PreUpdate))
            // 1) Perception & facing in-order BEFORE scorers (register once)
            .add_systems(
                PreUpdate,
                (sense_player, face_by_target_or_velocity)
                    .chain()
                    .before(BigBrainSet::Scorers),
            )
            // 2) Scorers & actions
            .add_systems(
                PreUpdate,
                (
                    has_target_scorer.in_set(BigBrainSet::Scorers),
                    attack_in_range_scorer.in_set(BigBrainSet::Scorers),
                    patrol_action.in_set(BigBrainSet::Actions),
                    chase_action.in_set(BigBrainSet::Actions),
                    attack_action.in_set(BigBrainSet::Actions),
                ),
            )
            // 3) Regular update helpers
            .add_systems(
                Update,
                (
                    tick_enemy_attack_timers,
                    on_enemy_added_attach_sprite_and_anims,
                    drive_enemy_animation,
                ),
            );
    }
}
