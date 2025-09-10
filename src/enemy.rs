// enemy.rs
use crate::animations::{DEFAULT_FRAME_MS, to_anim_name};
use crate::character::{GameLayer, Player};
use crate::enemy_class::{EnemyClass, EnemyClassAttachTarget};
use crate::gameflow::GameplayRoot;
use crate::raycasts::{MeleeAttackActive, MeleeRaycastHit, MeleeRaycastSpec};
use avian2d::collision::collider::{CollisionLayers, LayerMask};
use avian2d::prelude::*;
use avian2d::spatial_query::SpatialQueryFilter;
use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy_spritesheet_animation::prelude::*;
use big_brain::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;

// ──────────────────────────────────────────────────────────────────────────────
// Tiny JSON helper (same idea as in character.rs)
// ──────────────────────────────────────────────────────────────────────────────
#[derive(Deserialize)]
struct MiniAnim {
    name: String,
    last_col: usize,
}
#[derive(Deserialize)]
struct MiniManifest {
    animations: Vec<MiniAnim>,
}

fn load_anim_seconds_from_json(json_path: &str) -> HashMap<String, f32> {
    let mut map = HashMap::new();
    if let Ok(text) = std::fs::read_to_string(json_path) {
        if let Ok(manifest) = serde_json::from_str::<MiniManifest>(&text) {
            for a in manifest.animations {
                let pretty = to_anim_name(&a.name);
                let frames = a.last_col as u32;
                let secs = (frames * DEFAULT_FRAME_MS) as f32 / 1000.0;
                map.insert(pretty, secs);
            }
        }
    }
    map
}

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
    // NEW:
    pub stunned: Option<AnimationId>,
    pub die: Option<AnimationId>,
}

// OPTIONAL: Attack clip lengths so our swing timer matches the clip that plays
#[derive(Component, Clone, Copy)]
struct EnemyAttackDurations {
    idle: f32,
    walk: f32,
    run: f32,
    jump: f32,
    fall: f32,
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

// ====== Health / Impacts ======
#[derive(Component, Debug, Clone, Copy)]
pub struct EnemyStats {
    pub health: f32,
    pub _max_health: f32,
}

impl EnemyStats {
    pub fn new(max: f32) -> Self {
        Self {
            health: max,
            _max_health: max,
        }
    }
}

#[derive(Component, Default, Debug, Clone, Copy)]
struct EnemyLastHitDir(Vec2);

#[derive(Component, Default)]
struct EnemyStunned;

#[derive(Component, Default)]
struct EnemyDead;

#[derive(Component)]
struct EnemyStunTimer(Timer);

#[derive(Component)]
struct EnemyDeathTimer(Timer);

#[derive(Component, Clone, Copy)]
struct EnemyImpactDurations {
    stun: f32,
    die: f32,
}

// ====== Tuning ======
const WALK: f32 = 50.0;
const RUN: f32 = 200.0;
const ACCEL: f32 = 3000.0;
const AGGRO: f32 = 260.0;
const RANGE: f32 = 46.0;
// These remain fallback defaults; we’ll override from JSON when available.
const SWING_DEFAULT: f32 = 0.35;
const COOLDOWN: f32 = 0.60;

const ENEMY_KNOCKBACK_SPEED: f32 = 260.0;
const ENEMY_KNOCKBACK_POP: f32 = 300.0;

// ====== Bundle ======
#[derive(Bundle)]
pub struct EnemyBundle {
    enemy: Enemy,
    patrol: PatrolBounds,
    dir: PatrolDir,
    senses: EnemySenses,
    gameflow: GameplayRoot,

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
    ray: MeleeRaycastSpec,
    stats: EnemyStats,
    impacts: EnemyImpactDurations,
    class_target: EnemyClassAttachTarget,

    name: Name,
}

pub fn spawn_enemy(cmd: &mut Commands, pos: Vec2, left: f32, right: f32) -> Entity {
    let player_mask = SpatialQueryFilter::from_mask(LayerMask::from(GameLayer::Player));

    cmd.spawn(EnemyBundle {
        enemy: Enemy,
        gameflow: GameplayRoot,
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
        // Defaults; will be overwritten by JSON if available
        stats: EnemyStats::new(40.0),
        class_target: EnemyClassAttachTarget,
        impacts: EnemyImpactDurations {
            stun: 0.6,
            die: 1.2,
        },

        name: Name::new("Enemy"),
    })
    .insert(
        Thinker::build()
            .picker(FirstToScore::new(0.5))
            .when(AttackInRange, Attack)
            .when(HasTarget, Chase)
            .otherwise(Patrol),
    )
    .id()
}

// ====== Scorers ======
#[derive(Debug, Clone, Component, ScorerBuilder)]
pub struct HasTarget;

fn has_target_scorer(
    mut q: Query<(&Actor, &mut Score), With<HasTarget>>,
    senses: Query<&EnemySenses>,
    stuns: Query<Option<&EnemyStunned>>,
    deads: Query<Option<&EnemyDead>>,
) {
    for (Actor(actor), mut score) in q.iter_mut() {
        let disabled = stuns.get(*actor).ok().flatten().is_some()
            || deads.get(*actor).ok().flatten().is_some();
        if disabled {
            score.set(0.0);
            continue;
        }

        let has = senses.get(*actor).ok().and_then(|s| s.target).is_some();
        score.set(if has { 1.0 } else { 0.0 });
    }
}

#[derive(Debug, Clone, Component, ScorerBuilder)]
pub struct AttackInRange;

fn attack_in_range_scorer(
    mut q: Query<(&Actor, &mut Score), With<AttackInRange>>,
    senses: Query<&EnemySenses>,
    swinging_q: Query<Option<&MeleeAttackActive>>,
    cd_q: Query<Option<&EnemyAttackCooldown>>,
    stuns: Query<Option<&EnemyStunned>>,
    deads: Query<Option<&EnemyDead>>,
) {
    const ATTACK_BAND_X: f32 = RANGE + 24.0;

    for (Actor(actor), mut score) in q.iter_mut() {
        if stuns.get(*actor).ok().flatten().is_some() || deads.get(*actor).ok().flatten().is_some()
        {
            score.set(0.0);
            continue;
        }

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
    stuns: Query<Option<&EnemyStunned>>,
    deads: Query<Option<&EnemyDead>>,
) {
    for (Actor(actor), mut state) in q.iter_mut() {
        match *state {
            ActionState::Init | ActionState::Requested => {
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                if stuns.get(*actor).ok().flatten().is_some()
                    || deads.get(*actor).ok().flatten().is_some()
                {
                    continue;
                }

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
                *state = ActionState::Failure;
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
    stuns: Query<Option<&EnemyStunned>>,
    deads: Query<Option<&EnemyDead>>,
) {
    for (Actor(actor), mut state) in q.iter_mut() {
        match *state {
            ActionState::Init | ActionState::Requested => {
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                if stuns.get(*actor).ok().flatten().is_some()
                    || deads.get(*actor).ok().flatten().is_some()
                {
                    continue;
                }

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
                *state = ActionState::Failure;
            }
            ActionState::Success | ActionState::Failure => {
                *state = ActionState::Requested;
            }
        }
    }
}

fn on_enemy_class_added_set_hp(mut q: Query<(&EnemyClass, &mut EnemyStats), Added<EnemyClass>>) {
    for (class, mut stats) in &mut q {
        let max = class.0.base_stats.max_health as f32;
        stats._max_health = max;
        stats.health = max;
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
    senses_q: Query<&EnemySenses>,
    contacts_q: Query<&CollidingEntities>,
    durs_q: Query<&EnemyAttackDurations>,
    stuns: Query<Option<&EnemyStunned>>,
    deads: Query<Option<&EnemyDead>>,
) {
    for (Actor(actor), mut state) in q.iter_mut() {
        match *state {
            ActionState::Init | ActionState::Requested => {
                if stuns.get(*actor).ok().flatten().is_some()
                    || deads.get(*actor).ok().flatten().is_some()
                {
                    *state = ActionState::Failure;
                    continue;
                }

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
                    // === Pick an attack duration that matches the animation we’ll show ===
                    let d = durs_q.get(*actor).ok();
                    let v = vels.get_mut(*actor).ok();
                    let s = senses_q.get(*actor).ok();

                    let speed = v.as_ref().map(|v| v.x.abs()).unwrap_or(0.0);
                    let on_ground = contacts_q
                        .get(*actor)
                        .map(|c| !c.is_empty())
                        .unwrap_or(true);
                    let in_air = !on_ground;
                    let running = speed > (RUN * 0.7);
                    let moving = speed > 6.0;

                    let secs = if in_air {
                        // choose jump/fall by vertical sign if we had it; here use fall as fallback
                        d.map(|d| {
                            if v.as_ref().map(|v| v.y).unwrap_or(0.0) > 0.0 {
                                d.jump
                            } else {
                                d.fall
                            }
                        })
                        .unwrap_or(SWING_DEFAULT)
                    } else if running {
                        d.map(|d| d.run).unwrap_or(SWING_DEFAULT)
                    } else if moving || s.map(|s| s.dx.abs() > 4.0).unwrap_or(false) {
                        d.map(|d| d.walk).unwrap_or(SWING_DEFAULT)
                    } else {
                        d.map(|d| d.idle).unwrap_or(SWING_DEFAULT)
                    };

                    cmd.entity(*actor).insert((
                        MeleeAttackActive,
                        EnemyAttackTimer(Timer::from_seconds(secs, TimerMode::Once)),
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
                // If stunned mid-swing: cancel, no cooldown.
                if stuns.get(*actor).ok().flatten().is_some()
                    || deads.get(*actor).ok().flatten().is_some()
                {
                    cmd.entity(*actor)
                        .remove::<MeleeAttackActive>()
                        .remove::<EnemyAttackTimer>();
                    if let Ok(mut v) = vels.get_mut(*actor) {
                        v.x = 0.0;
                    }
                    *state = ActionState::Failure;
                    continue;
                }

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
    mut q: Query<
        (
            &mut Sprite,
            &LinearVelocity,
            &EnemySenses,
            Option<&EnemyStunned>,
            Option<&EnemyDead>,
        ),
        With<Enemy>,
    >,
) {
    for (mut sprite, vel, senses, stunned, dead) in q.iter_mut() {
        if stunned.is_some() || dead.is_some() {
            continue;
        }
        let dir = if senses.target.is_some() {
            senses.dx.signum()
        } else {
            vel.x.signum()
        };
        if dir > 0.05 {
            sprite.flip_x = false;
        } else if dir < -0.05 {
            sprite.flip_x = true;
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
            // NEW:
            stunned: library.animation_with_name("player_combat:stunned"),
            die: library.animation_with_name("player:die"),
        };

        // Load precise durations from JSON (same source as player)
        let secs_map = load_anim_seconds_from_json("assets/PlayerSheet2.json");

        let secs_attack_idle = *secs_map
            .get("player_combat:standingslash")
            .unwrap_or(&SWING_DEFAULT);
        let secs_attack_walk = *secs_map
            .get("player_combat:swordrunslash")
            .unwrap_or(&secs_attack_idle);
        let secs_attack_run = *secs_map
            .get("player_combat:swordsprintslash")
            .unwrap_or(&secs_attack_walk);
        let secs_attack_jump = *secs_map
            .get("player_combat:airslashup")
            .unwrap_or(&secs_attack_idle);
        let secs_attack_fall = *secs_map
            .get("player_combat:airslashdown")
            .unwrap_or(&secs_attack_jump);

        let stun_secs = *secs_map
            .get("player_combat:stunned")
            .or_else(|| secs_map.get("player:stunned"))
            .unwrap_or(&0.6);
        let die_secs = *secs_map.get("player:die").unwrap_or(&1.2);

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

        commands.entity(e).insert((
            sprite,
            anim,
            clips,
            EnemyCurrentAnim(idle_id),
            // attach attack durations & impact (stun/die) durations
            EnemyAttackDurations {
                idle: secs_attack_idle,
                walk: secs_attack_walk,
                run: secs_attack_run,
                jump: secs_attack_jump,
                fall: secs_attack_fall,
            },
            EnemyImpactDurations {
                stun: stun_secs,
                die: die_secs,
            },
        ));
    }
}

fn drive_enemy_animation(
    mut q: Query<
        (
            Entity,
            &EnemyAnimClips,
            &mut SpritesheetAnimation,
            &mut EnemyCurrentAnim,
            &LinearVelocity,
        ),
        With<Enemy>,
    >,
    stunned_q: Query<(), With<EnemyStunned>>,
    dead_q: Query<(), With<EnemyDead>>,
    swing_q: Query<(), With<MeleeAttackActive>>,
    contacts_q: Query<&CollidingEntities>,
) {
    for (e, clips, mut anim, mut current, vel) in &mut q {
        let dead = dead_q.get(e).is_ok();
        let stunned = stunned_q.get(e).is_ok();
        let swinging = swing_q.get(e).is_ok();

        let on_ground = contacts_q.get(e).map(|c| !c.is_empty()).unwrap_or(true);
        let in_air = !on_ground;
        let speed = vel.x.abs();
        let moving = speed > 6.0;
        let running = speed > (RUN * 0.7);

        let want = if dead {
            clips.die.or(Some(clips.idle))
        } else if stunned {
            clips.stunned.or(Some(clips.idle))
        } else if swinging {
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

                // If your animation type supports non-looping, consider disabling looping
                // for 'stunned' and 'die' here to avoid any rewind blip.
                // Example (uncomment if your type exposes this):
                // let non_loop = Some(id) == clips.die || Some(id) == clips.stunned;
                // anim.repeat = !non_loop;
                // or:
                // anim.mode = if non_loop { AnimationMode::OnceHoldLastFrame } else { AnimationMode::Repeat };

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

// ====== Damage & impacts ======

/// Apply damage to enemies and remember the hit direction (attacker → target).
fn apply_melee_damage_to_enemies(
    mut events: EventReader<MeleeRaycastHit>,
    mut enemies: Query<(Entity, &mut EnemyStats, Option<&Sprite>), With<Enemy>>,
    classes: Query<&EnemyClass>,
    xforms: Query<&GlobalTransform>,
    mut cmd: Commands,
) {
    for hit in events.read() {
        if let Ok((e, mut stats, _sprite)) = enemies.get_mut(hit.target) {
            let defense = classes
                .get(hit.target)
                .map(|c| c.0.base_stats.defense)
                .unwrap_or(0.0)
                .clamp(0.0, 0.95);

            let reduced = (hit.damage as f32) * (1.0 - defense);
            let dmg = reduced.max(0.0).ceil();
            stats.health = (stats.health - dmg).max(0.0);

            // Remember direction (attacker → target), used for knockback
            if let (Ok(att_tf), Ok(tgt_tf)) = (xforms.get(hit.attacker), xforms.get(hit.target)) {
                let d = tgt_tf.translation() - att_tf.translation();
                let dir = Vec2::new(d.x, d.y).normalize_or_zero();
                cmd.entity(e).insert(EnemyLastHitDir(dir));
            }
        }
    }
}

/// React to health changes: Stun on damage; Die on <= 0.
fn react_to_enemy_health_changes(
    mut cmd: Commands,
    q: Query<
        (
            Entity,
            &EnemyStats,
            &EnemyImpactDurations,
            Option<&EnemyDead>,
        ),
        With<Enemy>,
    >,
    mut last: Local<HashMap<Entity, f32>>,
) {
    for (e, stats, impacts, is_dead) in &q {
        let prev = last.get(&e).copied().unwrap_or(stats.health);
        last.insert(e, stats.health);

        if stats.health >= prev || is_dead.is_some() {
            continue;
        }

        // Disable hitbox while stunned/dead
        cmd.entity(e).remove::<MeleeAttackActive>();

        if stats.health <= 0.0 {
            cmd.entity(e)
                .remove::<EnemyStunned>()
                .remove::<EnemyStunTimer>()
                .insert(EnemyDead)
                .insert(EnemyDeathTimer(Timer::from_seconds(
                    impacts.die,
                    TimerMode::Once,
                )));
        } else {
            // Enter stun; knockback applied on Added<EnemyStunned>
            cmd.entity(e)
                .insert(EnemyStunned)
                .insert(EnemyStunTimer(Timer::from_seconds(
                    impacts.stun,
                    TimerMode::Once,
                )));
        }
    }
}

/// Apply knockback velocity on stun enter.
fn on_added_enemy_stunned_knockback(
    mut q: Query<
        (
            Entity,
            &mut LinearVelocity,
            Option<&EnemyLastHitDir>,
            Option<&Sprite>,
        ),
        Added<EnemyStunned>,
    >,
    classes: Query<&EnemyClass>,
) {
    for (e, mut vel, last_hit, sprite) in &mut q {
        let dir = if let Some(d) = last_hit {
            d.0
        } else {
            let facing_right = sprite.map(|s| !s.flip_x).unwrap_or(true);
            if facing_right {
                Vec2::new(-1.0, 0.2)
            } else {
                Vec2::new(1.0, 0.2)
            }
        };

        let x_sign = if dir.x.abs() >= 0.1 {
            dir.x.signum()
        } else {
            let facing_right = sprite.map(|s| !s.flip_x).unwrap_or(true);
            if facing_right { -1.0 } else { 1.0 }
        };

        let resist = classes
            .get(e)
            .map(|c| c.0.base_stats.knockback_resist)
            .unwrap_or(0.0)
            .clamp(0.0, 0.95);
        let mult = 1.0 - resist;

        vel.x = x_sign * ENEMY_KNOCKBACK_SPEED * mult;
        vel.y = vel.y.max(ENEMY_KNOCKBACK_POP * mult);
    }
}

/// Tick stun/death timers. End stun; despawn on death finish.
fn tick_enemy_impact_timers(
    time: Res<Time>,
    mut cmd: Commands,
    mut stuns: Query<(Entity, &mut EnemyStunTimer), With<EnemyStunned>>,
    mut deaths: Query<(Entity, &mut EnemyDeathTimer), With<EnemyDead>>,
) {
    for (e, mut t) in &mut stuns {
        t.0.tick(time.delta());
        if t.0.finished() {
            cmd.entity(e)
                .remove::<EnemyStunned>()
                .remove::<EnemyStunTimer>();
        }
    }
    for (e, mut t) in &mut deaths {
        t.0.tick(time.delta());
        if t.0.finished() {
            cmd.entity(e).despawn();
        }
    }
}

fn on_added_enemy_dead_make_passive(
    mut cmd: Commands,
    mut q: Query<(Entity, &mut Transform, &mut LinearVelocity), Added<EnemyDead>>,
) {
    for (e, mut t, mut vel) in &mut q {
        // Visual: sit behind the player a bit
        t.translation.z -= 0.5;

        // Physics: freeze & disable collision
        vel.x = 0.0;
        vel.y = 0.0;

        cmd.entity(e)
            .insert(RigidBody::Kinematic) // no forces/gravity
            .remove::<Collider>(); // no collisions with player/world
        // (Optional) also clear any lingering melee tag, just in case
        // .remove::<MeleeAttackActive>();
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
                    on_enemy_class_added_set_hp,
                    apply_melee_damage_to_enemies,
                    react_to_enemy_health_changes,
                    tick_enemy_impact_timers,
                ),
            )
            // 4) PostUpdate: apply stun knockback on tag add
            .add_systems(
                PostUpdate,
                (
                    on_added_enemy_stunned_knockback,
                    on_added_enemy_dead_make_passive,
                ),
            );
    }
}
