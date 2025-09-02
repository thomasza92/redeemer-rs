use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy_spritesheet_animation::prelude::*;
use leafwing_input_manager::prelude::*;
use avian2d::prelude::*;
use seldom_state::prelude::*;
use seldom_state::trigger::{just_pressed, value_unbounded};
use seldom_state::machine::TransCtx;

use crate::level::PassThroughOneWayPlatform;
use crate::animations::PlayerSpritesheet;

// ───────── Input ─────────
#[derive(Actionlike, Clone, Eq, Hash, PartialEq, Reflect, Debug)]
pub enum Action {
    #[actionlike(Axis)]
    Move,
    Jump,
    Attack,
    Sprint,
}

// ───────── States ────────
#[derive(Clone, Copy, Component, Reflect)]
#[component(storage = "SparseSet")]
pub enum Grounded {
    Left = -1,
    Idle = 0,
    Right = 1,
}

#[derive(Clone, Component, Reflect)]
#[component(storage = "SparseSet")]
pub struct Falling {
    pub velocity: f32, // initial jump velocity (0.0 for step-off)
    pub vel_x: f32,
}

// ───────── Animation ────────
#[derive(Component, Clone, Copy)]
struct CurrentAnim(AnimationId);

// ───────── Tuning ────────
pub const PLAYER_SPEED: f32 = 150.0;
pub const SPRINT_MULTIPLIER: f32 = 1.75;
pub const JUMP_VELOCITY: f32 = 500.0;

pub const COYOTE_TIME_MS: u64 = 120;
pub const JUMP_BUFFER_MS: u64 = 150;

pub const MAX_FALL_SPEED: f32 = 900.0;
pub const JUMP_CUT_MULT: f32 = 0.25;

pub const ACCEL_GROUND: f32 = 3500.0;
pub const DECEL_GROUND: f32 = 4000.0;
pub const ACCEL_AIR: f32   = 2000.0;
pub const DECEL_AIR: f32   = 2000.0;

const ASCEND_EPS: f32 = 10.0;
const LANDING_ASCEND_TOL: f32 = 10.0;

// ───────── Tags & data ─────────
#[derive(Component)]
pub struct Actor;

#[derive(Component, Clone, Copy)]
pub struct AnimClips {
    pub idle: AnimationId,
    pub walk: Option<AnimationId>,
    pub run:  Option<AnimationId>,
    pub jump: Option<AnimationId>,
    pub fall: Option<AnimationId>,
}

// Timers for leniency
#[derive(Component)]
struct CoyoteTimer(Timer);

#[derive(Component)]
struct JumpBuffer(Timer);

// Bundle to avoid giant spawn tuples
#[derive(Bundle)]
struct PlayerBundle {
    actor: Actor,
    clips: AnimClips,
    grounded: Grounded,
    machine: StateMachine,

    // Render / anim
    sprite: Sprite,
    anim: SpritesheetAnimation,
    collisions: CollidingEntities,

    // Physics
    body: RigidBody,
    lock: LockedAxes,
    restitution: Restitution,
    friction: Friction,
    damping: LinearDamping,
    collider: Collider,
    speculative: SpeculativeMargin,
    one_way: PassThroughOneWayPlatform,

    // Input
    input_map: InputMap<Action>,
    action_state: ActionState<Action>,

    // Transforms
    transform: Transform,
    global_transform: GlobalTransform,

    // Leniency helpers
    coyote: CoyoteTimer,
    jump_buffer: JumpBuffer,
}

// ───────── Spawner ─────────
pub fn spawn_main_character(
    mut commands: Commands,
    sheet: Res<PlayerSpritesheet>,
    library: Res<AnimationLibrary>,
) {
    // Anim IDs
    let idle_id = library
        .animation_with_name("player:idle")
        .expect("missing animation: player:idle");

    let clips = AnimClips {
        idle: idle_id,
        walk:  library.animation_with_name("player:walk"),
        run:   library.animation_with_name("player:run"),
        jump:  library.animation_with_name("player:jump"),
        fall:  library.animation_with_name("player:fall"),
    };

    // Sprite
    let mut sprite = Sprite::from_atlas_image(
        sheet.image.clone(),
        TextureAtlas { layout: sheet.layout.clone(), ..Default::default() },
    );
    sprite.anchor = Anchor::Custom(Vec2::new(0.0, -0.25));

    // Input map
    let input_map = InputMap::default()
        .with_axis(Action::Move, VirtualAxis::horizontal_arrow_keys())
        .with_axis(Action::Move, GamepadControlAxis::new(GamepadAxis::LeftStickX))
        .with(Action::Jump, KeyCode::Space)
        .with(Action::Jump, GamepadButton::South)
        .with(Action::Attack, KeyCode::KeyJ)
        .with(Action::Attack, GamepadButton::West)
        .with(Action::Sprint, KeyCode::ShiftLeft)
        .with(Action::Sprint, GamepadButton::LeftTrigger);

    // State machine: Jump (Grounded -> Falling), directional idle/walk/run mapping while grounded
    let machine = StateMachine::default()
        .trans::<Grounded, _>(
            just_pressed(Action::Jump),
            Falling { velocity: JUMP_VELOCITY, vel_x: 0.0 },
        )
        .trans_builder(value_unbounded(Action::Move), axis_to_grounded);

    let mut anim = SpritesheetAnimation::from_id(idle_id);
    anim.playing = true;

    let player = PlayerBundle {
        actor: Actor,
        clips,
        grounded: Grounded::Idle,
        machine,
        sprite,
        anim,
        collisions: CollidingEntities::default(),
        body: RigidBody::Dynamic,
        lock: LockedAxes::ROTATION_LOCKED,
        restitution: Restitution::ZERO.with_combine_rule(CoefficientCombine::Min),
        friction: Friction::ZERO.with_combine_rule(CoefficientCombine::Min),
        damping: LinearDamping(2.0),
        collider: Collider::capsule(8.0, 26.0),
        speculative: SpeculativeMargin(0.1),
        one_way: PassThroughOneWayPlatform::Never,
        input_map,
        action_state: ActionState::default(),
        transform: Transform::default(),
        global_transform: GlobalTransform::default(),
        coyote: CoyoteTimer(Timer::from_seconds(0.0, TimerMode::Once)),
        jump_buffer: JumpBuffer(Timer::from_seconds(0.0, TimerMode::Once)),
    };

    commands
        .spawn(player)
        .insert((
            Name::new("Player"),
            CurrentAnim(idle_id),
        ));
}

// ───────── seldom_state helpers ─────────
fn axis_to_grounded(In(t): In<TransCtx<Grounded, f32>>) -> Grounded {
    let v = t.out;
    if v > 0.5 { Grounded::Right }
    else if v < -0.5 { Grounded::Left }
    else { Grounded::Idle }
}

// ───────── Mechanics systems ─────────

// Record a buffered jump on press (consumed on landing or while coyote active)
fn record_jump_buffer(
    mut q: Query<(&ActionState<Action>, &mut JumpBuffer), With<Actor>>,
) {
    for (actions, mut buf) in &mut q {
        if actions.just_pressed(&Action::Jump) {
            buf.0 = Timer::from_seconds(JUMP_BUFFER_MS as f32 / 1000.0, TimerMode::Once);
        }
    }
}

// Advance timers
fn tick_timers(mut q: Query<(&mut CoyoteTimer, &mut JumpBuffer)>, time: Res<Time>) {
    for (mut coyote, mut buf) in &mut q {
        coyote.0.tick(time.delta());
        buf.0.tick(time.delta());
    }
}

// Step off ledge => start Falling (no upward impulse) and start coyote window
fn detect_step_off_ledge(
    mut commands: Commands,
    mut q: Query<(Entity, Option<&LinearVelocity>, &CollidingEntities, &mut CoyoteTimer),
                 (With<Actor>, With<Grounded>, Without<Falling>)>,
) {
    for (e, vel, contacts, mut coyote) in &mut q {
        let vy = vel.map(|v| v.y).unwrap_or(0.0);
        if contacts.is_empty() && vy <= 0.5 {
            let mut ec = commands.entity(e);
            ec.remove::<Grounded>();
            ec.insert(Falling { velocity: 0.0, vel_x: 0.0 });
            coyote.0 = Timer::from_seconds(COYOTE_TIME_MS as f32 / 1000.0, TimerMode::Once);
        }
    }
}

// Land if descending and colliding; clear Falling, set Grounded, stop coyote
fn detect_landing(
    mut commands: Commands,
    // make LinearVelocity mutable so we can zero vy
    mut q: Query<(Entity, &mut LinearVelocity, &CollidingEntities, &mut CoyoteTimer), With<Falling>>,
) {
    for (e, mut vel, contacts, mut coyote) in &mut q {
        // If touching something and we're not rising meaningfully, consider it landed
        if !contacts.is_empty() && vel.y <= LANDING_ASCEND_TOL {
            vel.y = 0.0; // kill residual bounce immediately
            let mut ec = commands.entity(e);
            ec.remove::<Falling>();
            ec.insert(Grounded::Idle);
            coyote.0 = Timer::from_seconds(0.0, TimerMode::Once);
        }
    }
}

// If buffered jump exists and we're grounded or within coyote window, jump now
fn consume_jump_buffer_and_jump(
    mut commands: Commands,
    q: Query<(Entity, Option<&Grounded>, &CoyoteTimer, &JumpBuffer), With<Actor>>,
) {
    for (e, grounded, coyote, buf) in &q {
        let buffer_active = !buf.0.finished();
        let can_jump = grounded.is_some() || !coyote.0.finished();
        if buffer_active && can_jump {
            let mut ec = commands.entity(e);
            ec.remove::<Grounded>();
            ec.insert(Falling { velocity: JUMP_VELOCITY, vel_x: 0.0 });
            ec.insert(JumpBuffer(Timer::from_seconds(0.0, TimerMode::Once)));
        }
    }
}

// One-shot vertical impulse when Falling is first added and velocity>0 (real jumps only)
fn apply_jump_impulse_on_added_falling(
    mut q: Query<(&Falling, &mut LinearVelocity), Added<Falling>>,
) {
    for (falling, mut vel) in &mut q {
        if falling.velocity > 0.0 {
            vel.y = falling.velocity;
        }
    }
}

// Variable jump height: cut vertical speed on jump release if still ascending
fn cut_jump_on_release(
    mut q: Query<(&ActionState<Action>, &mut LinearVelocity), With<Falling>>,
) {
    for (actions, mut vel) in &mut q {
        if actions.just_released(&Action::Jump) && vel.y > 0.0 {
            vel.y *= JUMP_CUT_MULT;
        }
    }
}

// Clamp terminal fall speed
fn clamp_fall_speed(mut q: Query<&mut LinearVelocity, With<Actor>>) {
    for mut vel in &mut q {
        if vel.y < -MAX_FALL_SPEED {
            vel.y = -MAX_FALL_SPEED;
        }
    }
}

// Ground/Air acceleration model for responsive feel
fn drive_motion_accel(
    time: Res<Time>,
    mut q: Query<(&ActionState<Action>, Option<&Grounded>, &mut LinearVelocity), With<Actor>>,
) {
    for (actions, grounded, mut vel) in &mut q {
        let axis = actions.value(&Action::Move);
        let sprint = if actions.pressed(&Action::Sprint) { SPRINT_MULTIPLIER } else { 1.0 };
        let target = axis * PLAYER_SPEED * sprint;

        let (accel, decel) = if grounded.is_some() {
            (ACCEL_GROUND, DECEL_GROUND)
        } else {
            (ACCEL_AIR, DECEL_AIR)
        };

        let dt = time.delta_secs();
        let delta = target - vel.x;
        let rate = if target.abs() > 0.01 { accel } else { decel };
        let step = (rate * dt).min(delta.abs());
        vel.x += step * delta.signum();
    }
}

// Flip sprite by input axis (visual)
fn face_by_input(mut q: Query<(&ActionState<Action>, &mut Sprite), With<Actor>>) {
    for (actions, mut sprite) in &mut q {
        let axis = actions.value(&Action::Move);
        if axis > 0.3 { sprite.flip_x = false; }
        else if axis < -0.3 { sprite.flip_x = true; }
    }
}

// Drive animation from state/velocity with guarded switching
fn drive_animation(
    mut q: Query<(
        &AnimClips,
        &mut SpritesheetAnimation,
        Option<&Falling>,
        Option<&Grounded>,
        &LinearVelocity,
        &ActionState<Action>,
        &mut CurrentAnim,
        &CollidingEntities,                // ← NEW
    ), With<Actor>>,
) {
    for (clips, mut anim, falling, grounded, vel, actions, mut current, contacts) in &mut q {
        let want = if falling.is_some() {
            // Only show JUMP when rising and NOT colliding with anything.
            // If colliding (landing frame) or not rising enough, show FALL.
            if vel.y > ASCEND_EPS && contacts.is_empty() {
                clips.jump
            } else {
                clips.fall
            }
        } else if grounded.is_some() {
            let moving = actions.value(&Action::Move).abs() > 0.5;
            if moving {
                if actions.pressed(&Action::Sprint) { clips.run.or(clips.walk) } else { clips.walk }
            } else {
                Some(clips.idle)
            }
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

// ───────── Plugin wiring ─────────
pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app
            // UPDATE: inputs, movement, timers
            .add_systems(Update, (
                record_jump_buffer,
                drive_motion_accel,
                face_by_input,
                detect_step_off_ledge,
                cut_jump_on_release,
                clamp_fall_speed,
                tick_timers,
            ))
            // POSTUPDATE: resolve landing/consumed jump, then animate (strict order)
            .add_systems(PostUpdate, (detect_landing, consume_jump_buffer_and_jump, drive_animation).chain())
            // One-shot jump impulse after components are added
            .add_systems(PostUpdate, apply_jump_impulse_on_added_falling);
    }
}