use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy_spritesheet_animation::prelude::*;
use leafwing_input_manager::prelude::*;
use avian2d::prelude::*;
use seldom_state::prelude::*;
use seldom_state::trigger::just_pressed;

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
pub struct Falling;

// ───────── Animation ────────
#[derive(Component, Clone, Copy)]
struct CurrentAnim(AnimationId);

#[derive(Component, Clone, Copy)]
pub struct AnimClips {
    pub idle: AnimationId,
    pub walk: Option<AnimationId>,
    pub run:  Option<AnimationId>,
    pub jump: Option<AnimationId>,
    pub fall: Option<AnimationId>,
}

// ───────── Tuning ────────
const PLAYER_SPEED: f32 = 160.0;
const SPRINT_MULTIPLIER: f32 = 1.75;
const JUMP_VELOCITY: f32 = 520.0;

// ───────── Tags ─────────
#[derive(Component)]
pub struct Actor;

// ───────── Bundle ─────────
#[derive(Bundle)]
struct PlayerBundle {
    actor: Actor,
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

    // Input
    let input_map = InputMap::default()
        .with_axis(Action::Move, VirtualAxis::horizontal_arrow_keys())
        .with_axis(Action::Move, GamepadControlAxis::new(GamepadAxis::LeftStickX))
        .with(Action::Jump, KeyCode::Space)
        .with(Action::Jump, GamepadButton::South)
        .with(Action::Attack, KeyCode::KeyJ)
        .with(Action::Attack, GamepadButton::West)
        .with(Action::Sprint, KeyCode::ShiftLeft)
        .with(Action::Sprint, GamepadButton::LeftTrigger);

    // Anim
    let mut anim = SpritesheetAnimation::from_id(idle_id);
    anim.playing = true;

    // ───── Minimal triggers (In<Entity>)
    fn walking(In(e): In<Entity>, act_q: Query<&ActionState<Action>>) -> bool {
        if let Ok(a) = act_q.get(e) {
            a.value(&Action::Move).abs() >= 0.5 && !a.pressed(&Action::Sprint)
        } else { false }
    }
    fn sprinting(In(e): In<Entity>, act_q: Query<&ActionState<Action>>) -> bool {
        if let Ok(a) = act_q.get(e) {
            a.value(&Action::Move).abs() >= 0.5 && a.pressed(&Action::Sprint)
        } else { false }
    }
    fn stopped_moving(In(e): In<Entity>, act_q: Query<&ActionState<Action>>) -> bool {
        act_q.get(e).ok().map(|a| a.value(&Action::Move).abs() < 0.5).unwrap_or(false)
    }
    fn step_off(In(e): In<Entity>, contacts_q: Query<&CollidingEntities>) -> bool {
        contacts_q.get(e).ok().map(|c| c.is_empty()).unwrap_or(true)
    }
    fn landed(In(e): In<Entity>, contacts_q: Query<&CollidingEntities>) -> bool {
        contacts_q.get(e).ok().map(|c| !c.is_empty()).unwrap_or(false)
    }
    fn apex(In(e): In<Entity>, vel_q: Query<&LinearVelocity>, contacts_q: Query<&CollidingEntities>) -> bool {
        let in_air = contacts_q.get(e).ok().map(|c| c.is_empty()).unwrap_or(true);
        let vy = vel_q.get(e).ok().map(|v| v.y).unwrap_or(0.0);
        in_air && vy <= 0.0
    }

    // ───── Machine
    let machine = StateMachine::default()
        // IDLE
        .trans::<Idle, _>(just_pressed(Action::Jump), Jumping)
        .trans::<Idle, _>(sprinting, Running)
        .trans::<Idle, _>(walking, Walking)
        .trans::<Idle, _>(step_off, Falling)
        // WALKING
        .trans::<Walking, _>(just_pressed(Action::Jump), Jumping)
        .trans::<Walking, _>(sprinting, Running)
        .trans::<Walking, _>(stopped_moving, Idle)
        .trans::<Walking, _>(step_off, Falling)
        // RUNNING
        .trans::<Running, _>(just_pressed(Action::Jump), Jumping)
        .trans::<Running, _>(walking, Walking)
        .trans::<Running, _>(stopped_moving, Idle)
        .trans::<Running, _>(step_off, Falling)
        // AIR
        .trans::<Jumping, _>(apex, Falling)
        .trans::<Jumping, _>(landed, Idle)
        .trans::<Falling, _>(landed, Idle);

    commands
        .spawn(PlayerBundle {
            actor: Actor,
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

            transform: Transform::default(),
            global_transform: GlobalTransform::default(),
        })
        .insert(Name::new("Player"));
}

fn drive_motion_set_velocity(
    mut q: Query<(&ActionState<Action>, &mut LinearVelocity), With<Actor>>,
) {
    for (actions, mut vel) in &mut q {
        let axis = actions.value(&Action::Move);
        let sprint_mult = if actions.pressed(&Action::Sprint) { SPRINT_MULTIPLIER } else { 1.0 };
        vel.x = axis * PLAYER_SPEED * sprint_mult;
    }
}

fn on_added_jumping_set_impulse(
    mut q: Query<&mut LinearVelocity, Added<Jumping>>,
) {
    for mut vel in &mut q {
        vel.y = JUMP_VELOCITY;
    }
}

fn face_by_input(mut q: Query<(&ActionState<Action>, &mut Sprite), With<Actor>>) {
    for (actions, mut sprite) in &mut q {
        let axis = actions.value(&Action::Move);
        if axis > 0.1 { sprite.flip_x = false; }
        else if axis < -0.1 { sprite.flip_x = true; }
    }
}

// ───────── Animation (kept simple) ─────────
fn drive_animation(
    mut q: Query<(
        &AnimClips,
        &mut SpritesheetAnimation,
        &mut CurrentAnim,
        Option<&Idle>, Option<&Walking>, Option<&Running>,
        Option<&Jumping>, Option<&Falling>,
        &LinearVelocity,
    ), With<Actor>>,
) {
    for (clips, mut anim, mut current, idle, walking, running, jumping, falling, vel) in &mut q {
        let want = if jumping.is_some() {
            clips.jump.or(clips.fall).or(Some(clips.idle))
        } else if falling.is_some() {
            if vel.y > 0.0 { clips.jump.or(clips.fall) } else { clips.fall }.or(Some(clips.idle))
        } else if running.is_some() {
            clips.run.or(clips.walk).or(Some(clips.idle))
        } else if walking.is_some() {
            clips.walk.or(Some(clips.idle))
        } else if idle.is_some() {
            Some(clips.idle)
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

// ───────── Plugin ─────────
pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(Update, (
                drive_motion_set_velocity,
                face_by_input,
            ))
            .add_systems(PostUpdate, (
                on_added_jumping_set_impulse,
                drive_animation,
            ));
    }
}