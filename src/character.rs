use crate::animations::PlayerSpritesheet;
use crate::animations::{DEFAULT_FRAME_MS, to_anim_name};
use crate::class::*;
use crate::gameflow::GameplayRoot;
use crate::level::PassThroughOneWayPlatform;
use crate::prelude::*;
use crate::raycasts::{MeleeAttackActive, MeleeRaycastHit, MeleeRaycastSpec, RaycastMeleePlugin};
use avian2d::collision::collider::{CollisionLayers, LayerMask, PhysicsLayer};
use avian2d::spatial_query::SpatialQueryFilter;
use bevy::log::info;
use bevy::sprite::Anchor;
use seldom_state::trigger::just_pressed;
use serde::Deserialize;
use std::collections::HashMap;

// ───────── Raycast Layers ─────────
#[derive(PhysicsLayer, Default)]
pub enum GameLayer {
    #[default]
    Default,
    Player,
    Enemy,
}
// ───────── Input ─────────
#[derive(Actionlike, Clone, Eq, Hash, PartialEq, Reflect, Debug)]
pub enum Action {
    #[actionlike(Axis)]
    Move,
    Jump,
    Attack,
    Sprint,
}

// ───────── States ─────────
#[derive(Component, Reflect, Default, Debug, Clone)]
#[component(storage = "SparseSet")]
pub struct Idle;

#[derive(Component, Reflect, Default, Debug, Clone)]
#[component(storage = "SparseSet")]
pub struct Walking;

#[derive(Component, Reflect, Default, Debug, Clone)]
#[component(storage = "SparseSet")]
pub struct Running;

#[derive(Component, Reflect, Default, Debug, Clone)]
#[component(storage = "SparseSet")]
pub struct Jumping;

#[derive(Component, Reflect, Default, Debug, Clone)]
#[component(storage = "SparseSet")]
pub struct SprintJumping;

#[derive(Component, Reflect, Default, Debug, Clone)]
#[component(storage = "SparseSet")]
pub struct Falling;

// Attack states & animation handling

#[derive(Component, Reflect, Default, Debug, Clone)]
#[component(storage = "SparseSet")]
pub struct IdleAttack;

#[derive(Component, Reflect, Default, Debug, Clone)]
#[component(storage = "SparseSet")]
pub struct WalkingAttack;

#[derive(Component, Reflect, Default, Debug, Clone)]
#[component(storage = "SparseSet")]
pub struct RunningAttack;

#[derive(Component, Reflect, Default, Debug, Clone)]
#[component(storage = "SparseSet")]
pub struct JumpingAttack;

#[derive(Component, Reflect, Default, Debug, Clone)]
#[component(storage = "SparseSet")]
pub struct FallingAttack;

#[derive(Component, Clone)]
struct AttackDurationsComp {
    idle: f32,
    walk: f32,
    run: f32,
    jump: f32,
    fall: f32,
}
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

// ───────── Animation ────────
#[derive(Component, Clone, Copy)]
struct CurrentAnim(AnimationId);

#[derive(Component, Clone, Copy)]
pub struct AnimClips {
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

// ───────── Tuning ────────
const PLAYER_SPEED: f32 = 160.0;
const SPRINT_MULTIPLIER: f32 = 1.75;
const JUMP_VELOCITY: f32 = 520.0;
const ATTACK_COOLDOWN_S: f32 = 0.15;

// ───────── Tags ─────────
#[derive(Component)]
pub struct Player;

// ───────── Attacks ─────────
#[derive(Component)]
struct AttackCooldown(Timer);

#[derive(Component)]
struct AttackTimer(Timer);

#[derive(Component)]
struct AttackDone;

// ───────── Bundle ─────────
#[derive(Bundle)]
struct PlayerBundle {
    player: Player,
    gameflow: GameplayRoot,
    class: ClassAttachTarget,
    machine: StateMachine,
    idle: Idle,
    sprite: Sprite,
    anim: SpritesheetAnimation,
    clips: AnimClips,
    current: CurrentAnim,
    body: RigidBody,
    lock: LockedAxes,
    restitution: Restitution,
    friction: Friction,
    damping: LinearDamping,
    collider: Collider,
    speculative: SpeculativeMargin,
    collisions: CollidingEntities,
    one_way: PassThroughOneWayPlatform,
    input_map: InputMap<Action>,
    action_state: ActionState<Action>,
    transform: Transform,
    global_transform: GlobalTransform,
}

// ───────── Spawner ─────────
pub fn spawn_main_character(
    mut commands: Commands,
    sheet: Res<PlayerSpritesheet>,
    library: Res<AnimationLibrary>,
) {
    // Anim IDs
    let idle_id = library
        .animation_with_name("player_combat:swordidle")
        .expect("missing animation: player_combat:swordidle");

    let clips = AnimClips {
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

    // Sprite
    let mut sprite = Sprite::from_atlas_image(
        sheet.image.clone(),
        TextureAtlas {
            layout: sheet.layout.clone(),
            ..Default::default()
        },
    );
    sprite.anchor = Anchor::Custom(Vec2::new(0.0, -0.3));

    // Input
    let input_map = InputMap::default()
        .with_axis(Action::Move, VirtualAxis::new(KeyCode::KeyA, KeyCode::KeyD))
        .with_axis(
            Action::Move,
            GamepadControlAxis::new(GamepadAxis::LeftStickX),
        )
        .with(Action::Jump, KeyCode::Space)
        .with(Action::Jump, GamepadButton::South)
        .with(Action::Attack, KeyCode::KeyJ)
        .with(Action::Attack, GamepadButton::West)
        .with(Action::Sprint, KeyCode::ShiftLeft)
        .with(Action::Sprint, GamepadButton::LeftTrigger);

    // Anim
    let mut anim = SpritesheetAnimation::from_id(idle_id);
    anim.playing = true;
    let secs_map = load_anim_seconds_from_json("assets/PlayerSheet2.json");
    let dur_idle = *secs_map.get("player_combat:standingslash").unwrap_or(&0.5);
    let dur_walk = *secs_map
        .get("player_combat:swordrunslash")
        .unwrap_or(&dur_idle);
    let dur_run = *secs_map
        .get("player_combat:swordsprintslash")
        .unwrap_or(&dur_walk);
    let dur_jump = *secs_map
        .get("player_combat:airslashup")
        .unwrap_or(&dur_idle);
    let dur_fall = *secs_map
        .get("player_combat:airslashdown")
        .unwrap_or(&dur_jump);
    let attack_durs = AttackDurationsComp {
        idle: dur_idle,
        walk: dur_walk,
        run: dur_run,
        jump: dur_jump,
        fall: dur_fall,
    };

    // Triggers
    fn walking(In(e): In<Entity>, act_q: Query<&ActionState<Action>>) -> bool {
        if let Ok(a) = act_q.get(e) {
            a.value(&Action::Move).abs() >= 0.5 && !a.pressed(&Action::Sprint)
        } else {
            false
        }
    }
    fn sprinting(In(e): In<Entity>, act_q: Query<&ActionState<Action>>) -> bool {
        if let Ok(a) = act_q.get(e) {
            a.value(&Action::Move).abs() >= 0.5 && a.pressed(&Action::Sprint)
        } else {
            false
        }
    }
    fn stopped_moving(
        In(e): In<Entity>,
        act_q: Query<&ActionState<Action>>,
        vel_q: Query<&LinearVelocity>,
    ) -> bool {
        let axis = act_q
            .get(e)
            .ok()
            .map(|a| a.value(&Action::Move))
            .unwrap_or(0.0);
        let vx = vel_q.get(e).ok().map(|v| v.x).unwrap_or(0.0);
        if axis.abs() >= 0.10 && vx.abs() > 8.0 && axis.signum() != vx.signum() {
            return false;
        }
        let input_small = axis.abs() < 0.10;
        let speed_small = vx.abs() < 8.0;
        input_small && speed_small
    }
    fn step_off(In(e): In<Entity>, contacts_q: Query<&CollidingEntities>) -> bool {
        contacts_q.get(e).ok().map(|c| c.is_empty()).unwrap_or(true)
    }
    fn landed(
        In(e): In<Entity>,
        contacts_q: Query<&CollidingEntities>,
        vel_q: Query<&LinearVelocity>,
        falling_q: Query<&Falling>,
    ) -> bool {
        let touching = contacts_q
            .get(e)
            .ok()
            .map(|c| !c.is_empty())
            .unwrap_or(false);
        if !touching {
            return false;
        }
        let vy = vel_q.get(e).ok().map(|v| v.y).unwrap_or(0.0);
        let is_falling = falling_q.get(e).is_ok();
        is_falling || vy <= 0.0
    }
    fn landed_walking(
        In(e): In<Entity>,
        act_q: Query<&ActionState<Action>>,
        contacts_q: Query<&CollidingEntities>,
        vel_q: Query<&LinearVelocity>,
        falling_q: Query<&Falling>,
    ) -> bool {
        let touching = contacts_q
            .get(e)
            .ok()
            .map(|c| !c.is_empty())
            .unwrap_or(false);
        if !touching {
            return false;
        }
        let vy = vel_q.get(e).ok().map(|v| v.y).unwrap_or(0.0);
        let is_falling = falling_q.get(e).is_ok();
        let landed_now = is_falling || vy <= 0.0;
        landed_now
            && act_q
                .get(e)
                .ok()
                .map(|a| a.value(&Action::Move).abs() >= 0.5 && !a.pressed(&Action::Sprint))
                .unwrap_or(false)
    }
    fn landed_sprinting(
        In(e): In<Entity>,
        act_q: Query<&ActionState<Action>>,
        contacts_q: Query<&CollidingEntities>,
        vel_q: Query<&LinearVelocity>,
        falling_q: Query<&Falling>,
    ) -> bool {
        let touching = contacts_q
            .get(e)
            .ok()
            .map(|c| !c.is_empty())
            .unwrap_or(false);
        if !touching {
            return false;
        }
        let vy = vel_q.get(e).ok().map(|v| v.y).unwrap_or(0.0);
        let is_falling = falling_q.get(e).is_ok();
        let landed_now = is_falling || vy <= 0.0;
        landed_now
            && act_q
                .get(e)
                .ok()
                .map(|a| a.value(&Action::Move).abs() >= 0.5 && a.pressed(&Action::Sprint))
                .unwrap_or(false)
    }
    fn apex(
        In(e): In<Entity>,
        vel_q: Query<&LinearVelocity>,
        contacts_q: Query<&CollidingEntities>,
    ) -> bool {
        let in_air = contacts_q.get(e).ok().map(|c| c.is_empty()).unwrap_or(true);
        let vy = vel_q.get(e).ok().map(|v| v.y).unwrap_or(0.0);
        in_air && vy <= 0.0
    }

    // Attack triggers
    fn attack_pressed_and_ready(
        In(e): In<Entity>,
        act_q: Query<&ActionState<Action>>,
        cd_q: Query<&AttackCooldown>,
    ) -> bool {
        if let (Ok(a), Ok(cd)) = (act_q.get(e), cd_q.get(e)) {
            a.just_pressed(&Action::Attack) && cd.0.finished()
        } else {
            false
        }
    }
    fn attack_finished(In(e): In<Entity>, q: Query<&AttackDone>) -> bool {
        q.get(e).is_ok()
    }
    fn attack_finished_walking(
        In(e): In<Entity>,
        done_q: Query<&AttackDone>,
        act_q: Query<&ActionState<Action>>,
    ) -> bool {
        done_q.get(e).is_ok()
            && act_q
                .get(e)
                .ok()
                .map(|a| a.value(&Action::Move).abs() >= 0.5 && !a.pressed(&Action::Sprint))
                .unwrap_or(false)
    }
    fn attack_finished_sprinting(
        In(e): In<Entity>,
        done_q: Query<&AttackDone>,
        act_q: Query<&ActionState<Action>>,
    ) -> bool {
        done_q.get(e).is_ok()
            && act_q
                .get(e)
                .ok()
                .map(|a| a.value(&Action::Move).abs() >= 0.5 && a.pressed(&Action::Sprint))
                .unwrap_or(false)
    }

    // ───── Machine
    let machine = StateMachine::default()
        // IDLE
        .trans::<Idle, _>(just_pressed(Action::Jump), Jumping)
        .trans::<Idle, _>(attack_pressed_and_ready, IdleAttack)
        .trans::<Idle, _>(sprinting, Running)
        .trans::<Idle, _>(walking, Walking)
        .trans::<Idle, _>(step_off, Falling)
        // WALKING
        .trans::<Walking, _>(just_pressed(Action::Jump), Jumping)
        .trans::<Walking, _>(attack_pressed_and_ready, WalkingAttack)
        .trans::<Walking, _>(sprinting, Running)
        .trans::<Walking, _>(stopped_moving, Idle)
        .trans::<Walking, _>(step_off, Falling)
        // RUNNING
        .trans::<Running, _>(just_pressed(Action::Jump), SprintJumping)
        .trans::<Running, _>(attack_pressed_and_ready, RunningAttack)
        .trans::<Running, _>(walking, Walking)
        .trans::<Running, _>(stopped_moving, Idle)
        .trans::<Running, _>(step_off, Falling)
        // AIR (base)
        .trans::<Jumping, _>(attack_pressed_and_ready, JumpingAttack)
        .trans::<Jumping, _>(apex, Falling)
        .trans::<Jumping, _>(landed_sprinting, Running)
        .trans::<Jumping, _>(landed_walking, Walking)
        .trans::<Jumping, _>(landed, Idle)
        .trans::<SprintJumping, _>(attack_pressed_and_ready, JumpingAttack)
        .trans::<SprintJumping, _>(apex, Falling)
        .trans::<SprintJumping, _>(landed_sprinting, Running)
        .trans::<SprintJumping, _>(landed_walking, Walking)
        .trans::<SprintJumping, _>(landed, Idle)
        .trans::<Falling, _>(attack_pressed_and_ready, FallingAttack)
        .trans::<Falling, _>(landed_sprinting, Running)
        .trans::<Falling, _>(landed_walking, Walking)
        .trans::<Falling, _>(landed, Idle)
        // ATTACK (ground) — keep attack while moving; exit when timer finishes
        .trans::<IdleAttack, _>(attack_finished_sprinting, Running)
        .trans::<IdleAttack, _>(attack_finished_walking, Walking)
        .trans::<IdleAttack, _>(attack_finished, Idle)
        .trans::<IdleAttack, _>(sprinting, RunningAttack)
        .trans::<IdleAttack, _>(walking, WalkingAttack)
        .trans::<IdleAttack, _>(step_off, FallingAttack)
        .trans::<WalkingAttack, _>(attack_finished_sprinting, Running)
        .trans::<WalkingAttack, _>(attack_finished_walking, Walking)
        .trans::<WalkingAttack, _>(attack_finished, Idle)
        .trans::<WalkingAttack, _>(sprinting, RunningAttack)
        .trans::<WalkingAttack, _>(stopped_moving, IdleAttack)
        .trans::<WalkingAttack, _>(step_off, FallingAttack)
        .trans::<RunningAttack, _>(attack_finished_sprinting, Running)
        .trans::<RunningAttack, _>(attack_finished_walking, Walking)
        .trans::<RunningAttack, _>(attack_finished, Idle)
        .trans::<RunningAttack, _>(walking, WalkingAttack)
        .trans::<RunningAttack, _>(stopped_moving, IdleAttack)
        .trans::<RunningAttack, _>(step_off, FallingAttack)
        // ATTACK (air) — follow air logic; exit to air base when timer ends
        .trans::<JumpingAttack, _>(attack_finished, Jumping)
        .trans::<JumpingAttack, _>(apex, FallingAttack)
        .trans::<JumpingAttack, _>(landed_sprinting, RunningAttack)
        .trans::<JumpingAttack, _>(landed_walking, WalkingAttack)
        .trans::<JumpingAttack, _>(landed, IdleAttack)
        .trans::<FallingAttack, _>(attack_finished, Falling)
        .trans::<FallingAttack, _>(landed_sprinting, RunningAttack)
        .trans::<FallingAttack, _>(landed_walking, WalkingAttack)
        .trans::<FallingAttack, _>(landed, IdleAttack);

    let enemy_mask = SpatialQueryFilter::from_mask(LayerMask::from(GameLayer::Enemy));

    let entity = commands
        .spawn(PlayerBundle {
            player: Player,
            gameflow: GameplayRoot,
            class: ClassAttachTarget,
            machine,
            idle: Idle,
            sprite,
            anim,
            clips,
            current: CurrentAnim(idle_id),
            body: RigidBody::Dynamic,
            lock: LockedAxes::ROTATION_LOCKED,
            restitution: Restitution::ZERO.with_combine_rule(CoefficientCombine::Min),
            friction: Friction::ZERO.with_combine_rule(CoefficientCombine::Min),
            damping: LinearDamping(2.0),
            collider: Collider::capsule(8.0, 26.0),
            speculative: SpeculativeMargin(0.1),
            collisions: CollidingEntities::default(),
            one_way: PassThroughOneWayPlatform::Never,
            input_map,
            action_state: ActionState::default(),
            transform: Transform::from_xyz(0., 0., -1.),
            global_transform: GlobalTransform::default(),
        })
        .insert(MeleeRaycastSpec {
            offset: Vec2::new(18.0, 8.0),
            length: 46.0,
            max_hits: 1,
            damage: 1,
            filter: enemy_mask,
            solid: false,
            once_per_swing: true,
        })
        .insert(attack_durs)
        .insert(Name::new("Player"))
        .insert(CollisionLayers::new(
            LayerMask::from(GameLayer::Player),
            LayerMask::from(GameLayer::Enemy) | LayerMask::from(GameLayer::Default),
        ))
        .id();
    commands
        .entity(entity)
        .insert(AttackCooldown(Timer::from_seconds(0.0, TimerMode::Once)));
}

// ───────── Motion ─────────
fn drive_motion_set_velocity(
    time: Res<Time>,
    mut q: Query<
        (
            &ActionState<Action>,
            &mut LinearVelocity,
            Option<&Jumping>,
            Option<&Falling>,
            Option<&SprintJumping>,
        ),
        With<Player>,
    >,
) {
    for (actions, mut vel, jumping, falling, sprint_jumping) in &mut q {
        let axis = actions.value(&Action::Move);
        let in_air = jumping.is_some() || falling.is_some() || sprint_jumping.is_some();
        let base_speed_mag = axis.abs() * PLAYER_SPEED;
        let already_above_base = vel.x.abs() > base_speed_mag;
        let sprint_mult = if sprint_jumping.is_some()
            || (falling.is_some() && already_above_base)
            || (!in_air && actions.pressed(&Action::Sprint))
        {
            SPRINT_MULTIPLIER
        } else {
            1.0
        };
        let target = axis * PLAYER_SPEED * sprint_mult;
        let accel = if in_air { 1800.0 } else { 3600.0 };
        let max_step = accel * time.delta_secs();
        let delta = (target - vel.x).clamp(-max_step, max_step);
        vel.x += delta;
    }
}

fn on_added_jumping_set_impulse(
    mut q: Query<&mut LinearVelocity, Or<(Added<Jumping>, Added<SprintJumping>)>>,
) {
    for mut vel in &mut q {
        vel.y = JUMP_VELOCITY;
    }
}

fn face_by_input(mut q: Query<(&ActionState<Action>, &mut Sprite), With<Player>>) {
    for (actions, mut sprite) in &mut q {
        let axis = actions.value(&Action::Move);
        if axis > 0.1 {
            sprite.flip_x = false;
        } else if axis < -0.1 {
            sprite.flip_x = true;
        }
    }
}

// ───────── Attack timers/cooldowns ─────────
fn tick_attack_timers(
    time: Res<Time>,
    mut q_cd: Query<&mut AttackCooldown, With<Player>>,
    mut q_atk: Query<&mut AttackTimer, With<Player>>,
) {
    for mut cd in &mut q_cd {
        cd.0.tick(time.delta());
    }
    for mut t in &mut q_atk {
        t.0.tick(time.delta());
    }
}

fn on_enter_attack_start_timer(
    mut commands: Commands,
    q_added: Query<
        Entity,
        Or<(
            Added<IdleAttack>,
            Added<WalkingAttack>,
            Added<RunningAttack>,
            Added<JumpingAttack>,
            Added<FallingAttack>,
        )>,
    >,
    q_state: Query<
        (
            Option<&IdleAttack>,
            Option<&WalkingAttack>,
            Option<&RunningAttack>,
            Option<&JumpingAttack>,
            Option<&FallingAttack>,
        ),
        With<Player>,
    >,
    _q_clips: Query<&AnimClips, With<Player>>,
    q_durs: Query<&AttackDurationsComp, With<Player>>,
    mut q_cd: Query<&mut AttackCooldown>,
) {
    for e in &q_added {
        let d = q_durs.get(e).ok().cloned().unwrap_or(AttackDurationsComp {
            idle: 0.5,
            walk: 0.5,
            run: 0.5,
            jump: 0.5,
            fall: 0.5,
        });
        let (idle_a, walk_a, run_a, jump_a, fall_a) = q_state
            .get(e)
            .ok()
            .unwrap_or((None, None, None, None, None));
        let secs = if idle_a.is_some() {
            d.idle
        } else if walk_a.is_some() {
            d.walk
        } else if run_a.is_some() {
            d.run
        } else if jump_a.is_some() {
            d.jump
        } else if fall_a.is_some() {
            d.fall
        } else {
            d.idle
        };

        commands
            .entity(e)
            .insert(AttackTimer(Timer::from_seconds(secs, TimerMode::Once)));

        if let Ok(mut cd) = q_cd.get_mut(e) {
            cd.0.reset();
            cd.0.set_duration(std::time::Duration::from_secs_f32(0.0));
        } else {
            commands
                .entity(e)
                .insert(AttackCooldown(Timer::from_seconds(0.0, TimerMode::Once)));
        }
        commands.entity(e).remove::<AttackDone>();
    }
}

fn finish_attack_when_timer_done(
    mut commands: Commands,
    mut q: Query<(Entity, &AttackTimer, Option<&mut AttackCooldown>)>,
) {
    for (e, timer, cd) in &mut q {
        if timer.0.finished() {
            let secs = ATTACK_COOLDOWN_S;
            if let Some(mut c) = cd {
                c.0.set_duration(std::time::Duration::from_secs_f32(secs));
                c.0.reset();
            } else {
                commands
                    .entity(e)
                    .insert(AttackCooldown(Timer::from_seconds(secs, TimerMode::Once)));
            }
            commands
                .entity(e)
                .insert(AttackDone)
                .remove::<AttackTimer>();
        }
    }
}

fn clear_attack_done(
    mut commands: Commands,
    q: Query<
        Entity,
        (
            With<AttackDone>,
            Without<IdleAttack>,
            Without<WalkingAttack>,
            Without<RunningAttack>,
            Without<JumpingAttack>,
            Without<FallingAttack>,
        ),
    >,
) {
    for e in &q {
        commands.entity(e).remove::<AttackDone>();
    }
}

// ───────── Animation ─────────
fn drive_animation(
    mut q: Query<
        (
            &AnimClips,
            &mut SpritesheetAnimation,
            &mut CurrentAnim,
            Option<&Idle>,
            Option<&Walking>,
            Option<&Running>,
            Option<&Jumping>,
            Option<&Falling>,
            Option<&SprintJumping>,
            Option<&IdleAttack>,
            Option<&WalkingAttack>,
            Option<&RunningAttack>,
            Option<&JumpingAttack>,
            Option<&FallingAttack>,
            &LinearVelocity,
        ),
        With<Player>,
    >,
) {
    for (
        clips,
        mut anim,
        mut current,
        _idle,
        walking,
        running,
        jumping,
        falling,
        sprint_jumping,
        idle_a,
        walking_a,
        running_a,
        jumping_a,
        falling_a,
        vel,
    ) in &mut q
    {
        // Attack takes precedence; pick specific attack clip per state
        let want = if let Some(_) = idle_a {
            Some(clips.attack_idle)
        } else if let Some(_) = walking_a {
            clips.attack_walk.or(Some(clips.attack_idle))
        } else if let Some(_) = running_a {
            clips
                .attack_run
                .or(clips.attack_walk)
                .or(Some(clips.attack_idle))
        } else if let Some(_) = jumping_a {
            clips.attack_jump.or(Some(clips.attack_idle))
        } else if let Some(_) = falling_a {
            clips
                .attack_fall
                .or(clips.attack_jump)
                .or(Some(clips.attack_idle))
        } else if sprint_jumping.is_some() || jumping.is_some() {
            clips.jump.or(clips.fall).or(Some(clips.idle))
        } else if falling.is_some() {
            if vel.y > 0.0 {
                clips.jump.or(clips.fall)
            } else {
                clips.fall
            }
            .or(Some(clips.idle))
        } else if running.is_some() {
            clips.run.or(clips.walk).or(Some(clips.idle))
        } else if walking.is_some() {
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

// ───────── Debug ─────────
fn debug_log_player_state(
    q: Query<
        (
            Option<&Idle>,
            Option<&Walking>,
            Option<&Running>,
            Option<&Jumping>,
            Option<&SprintJumping>,
            Option<&Falling>,
            Option<&IdleAttack>,
            Option<&WalkingAttack>,
            Option<&RunningAttack>,
            Option<&JumpingAttack>,
            Option<&FallingAttack>,
        ),
        With<Player>,
    >,
    mut last: Local<Option<&'static str>>,
) {
    for (
        idle,
        walking,
        running,
        jumping,
        sprint_jump,
        falling,
        idle_a,
        walking_a,
        running_a,
        jumping_a,
        falling_a,
    ) in &q
    {
        let now: &'static str = if idle_a.is_some() {
            "IdleAttack"
        } else if walking_a.is_some() {
            "WalkingAttack"
        } else if running_a.is_some() {
            "RunningAttack"
        } else if jumping_a.is_some() {
            "JumpingAttack"
        } else if falling_a.is_some() {
            "FallingAttack"
        } else if sprint_jump.is_some() {
            "SprintJumping"
        } else if jumping.is_some() {
            "Jumping"
        } else if falling.is_some() {
            "Falling"
        } else if running.is_some() {
            "Running"
        } else if walking.is_some() {
            "Walking"
        } else if idle.is_some() {
            "Idle"
        } else {
            "Idle"
        };

        if last.as_deref() != Some(now) {
            info!("Player state → {}", now);
            *last = Some(now);
        }
    }
}

pub fn bridge_attack_states_to_melee_tag(
    mut commands: Commands,
    q: Query<
        (
            Entity,
            Option<&IdleAttack>,
            Option<&WalkingAttack>,
            Option<&RunningAttack>,
            Option<&JumpingAttack>,
            Option<&FallingAttack>,
            Option<&MeleeAttackActive>,
        ),
        With<Player>,
    >,
) {
    for (e, idle_a, walk_a, run_a, jump_a, fall_a, melee_tag) in &q {
        let attacking = idle_a.is_some()
            || walk_a.is_some()
            || run_a.is_some()
            || jump_a.is_some()
            || fall_a.is_some();
        match (attacking, melee_tag.is_some()) {
            (true, false) => {
                commands.entity(e).insert(MeleeAttackActive);
            }
            (false, true) => {
                commands.entity(e).remove::<MeleeAttackActive>();
            }
            _ => {}
        }
    }
}
fn log_melee_hits(mut ev: EventReader<MeleeRaycastHit>) {
    for hit in ev.read() {
        info!(
            "Slash by {:?} hit {:?} at d={:.1} normal=({:.2},{:.2}) dmg={}",
            hit.attacker, hit.target, hit.distance, hit.normal.x, hit.normal.y, hit.damage
        );
    }
}

// ───────── Plugin ─────────
pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(RaycastMeleePlugin)
            .add_systems(
                Update,
                (
                    drive_motion_set_velocity,
                    face_by_input,
                    debug_log_player_state,
                    tick_attack_timers,
                    on_enter_attack_start_timer,
                    finish_attack_when_timer_done,
                    clear_attack_done,
                    bridge_attack_states_to_melee_tag,
                    log_melee_hits,
                ),
            )
            .add_systems(PostUpdate, (on_added_jumping_set_impulse, drive_animation));
    }
}
