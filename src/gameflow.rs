use crate::prelude::*;
use bevy::app::AppExit;
use bevy::time::Virtual;
use bevy::ui::GlobalZIndex;

#[derive(States, Debug, Hash, PartialEq, Eq, Clone, Copy, Default)]
pub enum GameState {
    #[default]
    MainMenu,
    InGame,
    Paused,
    Settings,
    GameOver,
}

#[derive(Event, Default)]
pub struct PlayerDied;

#[derive(Resource, Clone, Copy, Default)]
struct SettingsBackTarget(GameState);

#[derive(Component)]
pub struct GameplayRoot;

#[derive(Component)]
struct MainMenuUI;

#[derive(Component)]
struct MainMenuBg;

#[derive(Component)]
struct PauseMenuUI;

#[derive(Component)]
struct SettingsUI;

#[derive(Component)]
struct GameOverUI;

#[derive(Component)]
#[allow(dead_code)]
struct MenuBgLoop(Handle<vleue_kinetoscope::AnimatedImage>);

// Button tags
#[derive(Component, Clone, Copy)]
enum MainBtn {
    NewGame,
    Settings,
    Quit,
}
#[derive(Component, Clone, Copy)]
enum PauseBtn {
    Resume,
    Settings,
    MainMenu,
}
#[derive(Component, Clone, Copy)]
enum SetBtn {
    Back,
}
#[derive(Component, Clone, Copy)]
enum OverBtn {
    TryAgain,
    MainMenu,
}

pub struct GameFlowPlugin;

impl Plugin for GameFlowPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>()
            .init_resource::<SettingsBackTarget>()
            .add_event::<PlayerDied>()
            // Menus
            .add_systems(OnEnter(GameState::MainMenu), spawn_main_menu)
            .add_systems(
                Update,
                main_menu_buttons.run_if(in_state(GameState::MainMenu)),
            )
            .add_systems(
                Update,
                size_menu_bg_to_window.run_if(in_state(GameState::MainMenu))
            )
            .add_systems(Update, size_menu_bg_to_window.run_if(in_state(GameState::Settings)))
            .add_systems(
                OnExit(GameState::MainMenu),
                despawn_ui::<MainMenuUI>
            )
            .add_systems(OnEnter(GameState::Settings), spawn_settings_menu)
            .add_systems(OnEnter(GameState::InGame), despawn_menu_bg)
            .add_systems(OnExit(GameState::Settings), despawn_ui::<SettingsUI>)
            .add_systems(
                Update,
                settings_buttons.run_if(in_state(GameState::Settings)),
            )
            .add_systems(OnEnter(GameState::Paused), (spawn_pause_menu, pause_time))
            .add_systems(
                OnExit(GameState::Paused),
                (despawn_ui::<PauseMenuUI>, resume_time),
            )
            .add_systems(
                Update,
                pause_menu_buttons.run_if(in_state(GameState::Paused)),
            )
            .add_systems(OnEnter(GameState::GameOver), spawn_game_over)
            .add_systems(OnExit(GameState::GameOver), despawn_ui::<GameOverUI>)
            .add_systems(
                Update,
                game_over_buttons.run_if(in_state(GameState::GameOver)),
            )
            // Pause toggles
            .add_systems(Update, esc_to_pause.run_if(in_state(GameState::InGame)))
            .add_systems(Update, esc_to_resume.run_if(in_state(GameState::Paused)))
            // Death -> GameOver
            .add_systems(Update, to_game_over_on_death);
    }
}

fn menu_root(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            GlobalZIndex(1000),
            BackgroundColor(Color::NONE),
        ))
        .id()
}

fn menu_panel(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Px(520.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(12.0),
                padding: UiRect::all(Val::Px(24.0)),
                align_items: AlignItems::Stretch,
                margin: UiRect { top: Val::Px(280.0), ..default() },
                ..default()
            },
            BackgroundColor(Color::srgb(0.08, 0.08, 0.12)),
        ))
        .id()
}

fn menu_title(commands: &mut Commands, font: Handle<Font>, text: &str) -> Entity {
    commands
        .spawn((
            Text::new(text),
            TextFont {
                font,
                font_size: 44.0,
                ..default()
            },
            TextColor(Color::WHITE),
        ))
        .id()
}

fn spawn_button<A: Component>(
    commands: &mut Commands,
    font: &Handle<Font>,
    label: &str,
    action: A,
) -> Entity {
    let btn = commands
        .spawn((
            Button,
            Node {
                height: Val::Px(48.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.15, 0.15, 0.25)),
            action,
        ))
        .id();

    let text = commands
        .spawn((
            Text::new(label),
            TextFont {
                font: font.clone(),
                font_size: 24.0,
                ..default()
            },
            TextColor(Color::WHITE),
        ))
        .id();

    commands.entity(btn).add_child(text);
    btn
}

fn spawn_main_menu(
    mut commands: Commands,
    assets: Res<AssetServer>,
    q_bg: Query<(), With<MainMenuBg>>,
) {
    if q_bg.is_empty() {
        let stream_handle: Handle<vleue_kinetoscope::StreamingAnimatedImage>
            = assets.load("ui/menu_bg.webp");
        commands.spawn((
            MainMenuBg,
            vleue_kinetoscope::StreamingAnimatedImageController::play(stream_handle),
            Transform::from_xyz(0.0, 0.0, -5.0),
        ));
    }

    let font = assets.load("fonts/GohuFont14NerdFontMono-Regular.ttf");
    let root = menu_root(&mut commands);
    let panel = menu_panel(&mut commands);

    commands.entity(root).insert(MainMenuUI);
    commands.entity(root).add_child(panel);

    let b_new  = spawn_button(&mut commands, &font, "New Game", MainBtn::NewGame);
    let b_set  = spawn_button(&mut commands, &font, "Settings", MainBtn::Settings);
    let b_quit = spawn_button(&mut commands, &font, "Quit",     MainBtn::Quit);

    commands.entity(panel).add_children(&[b_new, b_set, b_quit]);
}

fn spawn_settings_menu(mut commands: Commands, assets: Res<AssetServer>) {
    let font = assets.load("fonts/GohuFont14NerdFontMono-Regular.ttf");

    let root = menu_root(&mut commands);
    let panel = menu_panel(&mut commands);

    commands.entity(root).insert(SettingsUI);
    commands.entity(root).add_child(panel);

    let title = menu_title(&mut commands, font.clone(), "SETTINGS");
    let b_back = spawn_button(&mut commands, &font, "Back", SetBtn::Back);

    commands.entity(panel).add_child(title);
    commands.entity(panel).add_child(b_back);
}

fn spawn_pause_menu(mut commands: Commands, assets: Res<AssetServer>) {
    let font = assets.load("fonts/GohuFont14NerdFontMono-Regular.ttf");

    let root = menu_root(&mut commands);
    let panel = menu_panel(&mut commands);

    commands.entity(root).insert(PauseMenuUI);
    commands.entity(root).add_child(panel);

    let title  = menu_title(&mut commands, font.clone(), "PAUSED");
    let b_res  = spawn_button(&mut commands, &font, "Resume",    PauseBtn::Resume);
    let b_set  = spawn_button(&mut commands, &font, "Settings",  PauseBtn::Settings);
    let b_menu = spawn_button(&mut commands, &font, "Main Menu", PauseBtn::MainMenu);

    commands.entity(panel).add_child(title);
    commands.entity(panel).add_children(&[b_res, b_set, b_menu]);
}

fn spawn_game_over(mut commands: Commands, assets: Res<AssetServer>) {
    let font = assets.load("fonts/GohuFont14NerdFontMono-Regular.ttf");

    let root = menu_root(&mut commands);
    let panel = menu_panel(&mut commands);

    commands.entity(root).insert(GameOverUI);
    commands.entity(root).add_child(panel);

    let title  = menu_title(&mut commands, font.clone(), "GAME OVER");
    let b_try  = spawn_button(&mut commands, &font, "Try Again", OverBtn::TryAgain);
    let b_menu = spawn_button(&mut commands, &font, "Main Menu", OverBtn::MainMenu);

    commands.entity(panel).add_child(title);
    commands.entity(panel).add_children(&[b_try, b_menu]);
}

fn set_btn_color(bg: &mut BackgroundColor, interaction: Interaction) {
    *bg = match interaction {
        Interaction::Pressed => Color::srgba(0.40, 0.40, 0.60, 1.0).into(),
        Interaction::Hovered => Color::srgba(0.25, 0.25, 0.40, 1.0).into(),
        Interaction::None    => Color::srgba(0.15, 0.15, 0.25, 1.0).into(),
    };
}

fn size_menu_bg_to_window(
    qwin: Query<&Window>,
    mut q: Query<&mut Sprite, With<MainMenuBg>>,
) {
    let Ok(win) = qwin.single() else { return; };
    let size = Vec2::new(win.width(), win.height());
    for mut sprite in &mut q {
        sprite.custom_size = Some(size);
    }
}

fn main_menu_buttons(
    mut next: ResMut<NextState<GameState>>,
    mut exit: EventWriter<AppExit>,
    mut back_target: ResMut<SettingsBackTarget>,
    mut q: Query<(&Interaction, &mut BackgroundColor, &MainBtn), (Changed<Interaction>, With<Button>)>,
) {
    for (i, mut bg, btn) in &mut q {
        set_btn_color(&mut bg, *i);
        if *i == Interaction::Pressed {
            match btn {
                MainBtn::NewGame  => next.set(GameState::InGame),
                MainBtn::Settings => {
                    back_target.0 = GameState::MainMenu;
                    next.set(GameState::Settings);
                }
                MainBtn::Quit     => { let _ = exit.write(AppExit::Success); }
            }
        }
    }
}

fn settings_buttons(
    mut next: ResMut<NextState<GameState>>,
    back_target: Res<SettingsBackTarget>,
    mut q: Query<(&Interaction, &mut BackgroundColor, &SetBtn), (Changed<Interaction>, With<Button>)>,
) {
    for (i, mut bg, btn) in &mut q {
        set_btn_color(&mut bg, *i);
        if *i == Interaction::Pressed {
            if matches!(btn, SetBtn::Back) {
                next.set(back_target.0);
            }
        }
    }
}

fn pause_menu_buttons(
    mut next: ResMut<NextState<GameState>>,
    mut back_target: ResMut<SettingsBackTarget>,
    mut q: Query<(&Interaction, &mut BackgroundColor, &PauseBtn), (Changed<Interaction>, With<Button>)>,
) {
    for (i, mut bg, btn) in &mut q {
        set_btn_color(&mut bg, *i);
        if *i == Interaction::Pressed {
            match btn {
                PauseBtn::Resume   => next.set(GameState::InGame),
                PauseBtn::Settings => {
                    back_target.0 = GameState::Paused;
                    next.set(GameState::Settings);
                }
                PauseBtn::MainMenu => next.set(GameState::MainMenu),
            }
        }
    }
}

fn game_over_buttons(
    mut next: ResMut<NextState<GameState>>,
    mut q: Query<
        (&Interaction, &mut BackgroundColor, &OverBtn),
        (Changed<Interaction>, With<Button>),
    >,
) {
    for (i, mut bg, btn) in &mut q {
        set_btn_color(&mut bg, *i);
        if *i == Interaction::Pressed {
            match btn {
                OverBtn::TryAgain => next.set(GameState::InGame),
                OverBtn::MainMenu => next.set(GameState::MainMenu),
            }
        }
    }
}

fn esc_to_pause(keys: Res<ButtonInput<KeyCode>>, mut next: ResMut<NextState<GameState>>) {
    if keys.just_pressed(KeyCode::Escape) {
        next.set(GameState::Paused);
    }
}
fn esc_to_resume(keys: Res<ButtonInput<KeyCode>>, mut next: ResMut<NextState<GameState>>) {
    if keys.just_pressed(KeyCode::Escape) {
        next.set(GameState::InGame);
    }
}
fn pause_time(mut time: ResMut<Time<Virtual>>) { time.pause(); }
fn resume_time(mut time: ResMut<Time<Virtual>>) { time.unpause(); }

fn to_game_over_on_death(
    mut ev: EventReader<PlayerDied>,
    mut next: ResMut<NextState<GameState>>,
) {
    if ev.read().next().is_some() {
        next.set(GameState::GameOver);
    }
}

fn despawn_ui<T: Component>(mut commands: Commands, q: Query<Entity, With<T>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

pub fn despawn_gameplay(mut commands: Commands, q: Query<Entity, With<GameplayRoot>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

fn despawn_menu_bg(mut commands: Commands, q: Query<Entity, With<MainMenuBg>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}