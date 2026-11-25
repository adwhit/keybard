//! Flash cards: the chord book's cleared challenges, dealt one at a time
//! against the clock (the model is [`crate::drill`]; this is its screen).
//!
//! Everything works from the piano: Dsus4 starts (and restarts) a round,
//! Bdim stops it, and the cards themselves are all major and minor triads,
//! so an answer can never press a button.

use std::time::{SystemTime, UNIX_EPOCH};

use bevy::prelude::*;
use tracing::info;

use super::book::{self, ChordBook};
use super::controls::Control;
use super::input::HeldKeys;
use super::menu::{self, Screen, describe_held};
use super::piano::GuideKeys;
use super::players::Profile;
use super::widgets::button;
use super::{Phase, palette};
use crate::drill::{Answer, Drill, Mistake, ROUND, Round, Stage, Summary, TIME_LIMIT};
use crate::layout;
use crate::music::MidiNote;
use crate::performance::Timing;
use crate::song::Seconds;

/// The drill, kept across rounds so the cards you miss stay favoured.
#[derive(Resource, Debug)]
pub struct FlashCards {
    pub drill: Drill,
}

const NOTHING_CLEARED: &str = "Nothing cleared yet: play this chord in each position above, and each one you get becomes a flash card.";

/// The time, as a seed: every run deals differently.
fn seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64)
}

#[derive(Component, Debug)]
struct DrillScreen;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
enum Line {
    Header,
    Name,
    Position,
    Clock,
    Feedback,
    Held,
    Summary,
    Details,
    WorkOn,
    Hint,
}

/// The shrinking bar that shows the time left.
#[derive(Component, Debug)]
struct ClockFill;

/// When a line or row is on screen: the others make way for it.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
enum Part {
    Always,
    /// While cards are being dealt.
    Dealing,
    /// Once the round is over.
    Finished,
}

impl Line {
    fn part(self) -> Part {
        match self {
            Self::Header | Self::Hint => Part::Always,
            Self::Name | Self::Position | Self::Clock | Self::Feedback | Self::Held => {
                Part::Dealing
            }
            Self::Summary | Self::Details | Self::WorkOn => Part::Finished,
        }
    }
}

pub fn plugin(app: &mut App) {
    app.insert_resource(FlashCards {
        drill: Drill::new(seed()),
    })
    .add_systems(Startup, spawn_screen)
    .add_systems(
        Update,
        (forget_on_player_change, steer)
            .chain()
            .in_set(Phase::Input)
            .after(menu::navigate),
    )
    .add_systems(
        Update,
        (run, show).chain().in_set(Phase::Present).after(book::show),
    );
}

/// Each player's misses are their own.
fn forget_on_player_change(profile: Res<Profile>, mut cards: ResMut<FlashCards>) {
    if profile.is_changed() && !profile.is_added() {
        cards.drill = Drill::new(seed());
    }
}

/// Deal a round from the book's cleared challenges, or say there are none.
fn deal(cards: &mut FlashCards, book: &mut ChordBook, screen: &mut Screen) {
    match cards.drill.start(book.progress()) {
        Ok(()) => *screen = Screen::Drill,
        Err(_) => {
            book.tell(NOTHING_CLEARED);
            *screen = Screen::ChordBook;
        }
    }
}

/// Start from the book, again once a round is over, and stop back to it.
fn steer(
    mut controls: MessageReader<Control>,
    mut screen: ResMut<Screen>,
    mut book: ResMut<ChordBook>,
    mut cards: ResMut<FlashCards>,
) {
    controls.read().for_each(|control| {
        let over = cards
            .drill
            .round()
            .is_some_and(|r| r.stage() == Stage::Over);
        match (*control, *screen) {
            (Control::StartDrill, Screen::ChordBook) => deal(&mut cards, &mut book, &mut screen),
            (Control::StartDrill, Screen::Drill) if over => {
                deal(&mut cards, &mut book, &mut screen);
            }
            (Control::Back, Screen::Drill) => {
                cards.drill.stop();
                *screen = Screen::ChordBook;
                info!("left the flash cards");
            }
            _ => {}
        }
    });
}

/// The chord is a button's (Stop, Again), not an answer.
fn is_button_chord(notes: &[MidiNote]) -> bool {
    [Control::Back, Control::StartDrill]
        .iter()
        .any(|c| c.chord().is_some_and(|k| k.matches(notes)))
}

/// Run the clock and mark what's played.
fn run(
    mut screen: ResMut<Screen>,
    mut book: ResMut<ChordBook>,
    mut cards: ResMut<FlashCards>,
    held: Res<HeldKeys>,
    time: Res<Time>,
) {
    if *screen != Screen::Drill {
        return;
    }
    // Opened straight onto the drill (e.g. from the command line).
    if cards.drill.round().is_none() {
        deal(&mut cards, &mut book, &mut screen);
        return;
    }
    let notes: Vec<MidiNote> = held.0.iter().copied().collect();
    if !is_button_chord(&notes) {
        cards.drill.play(&notes);
    }
    cards.drill.tick(Seconds(time.delta_secs()));
}

fn text(kind: Line, size: f32, color: Color) -> impl Bundle {
    (
        kind,
        kind.part(),
        Node::default(),
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color),
    )
}

fn row(margin: f32) -> Node {
    Node {
        flex_direction: FlexDirection::Row,
        column_gap: Val::Px(12.0),
        align_items: AlignItems::Center,
        margin: UiRect::top(Val::Px(margin)),
        ..default()
    }
}

const CLOCK_WIDTH: f32 = 420.0;

fn spawn_screen(mut commands: Commands) {
    commands.spawn((
        DrillScreen,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent((1.0 - layout::WHITE_KEY_HEIGHT / layout::CANVAS_HEIGHT) * 100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            padding: UiRect::top(Val::Percent(4.0)),
            row_gap: Val::Px(8.0),
            ..default()
        },
        BackgroundColor(palette::BACKGROUND),
        GlobalZIndex(30),
        Visibility::Hidden,
        children![
            text(Line::Header, 14.0, palette::HUD_DIM),
            text(Line::Name, 56.0, palette::timing(Timing::Perfect)),
            text(Line::Position, 24.0, palette::HUD_TEXT),
            (
                Part::Dealing,
                row(6.0),
                children![
                    (
                        Node {
                            width: Val::Px(CLOCK_WIDTH),
                            height: Val::Px(10.0),
                            ..default()
                        },
                        BackgroundColor(palette::PROGRESS_TRACK),
                        children![(
                            ClockFill,
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(palette::timing(Timing::Good)),
                        )],
                    ),
                    text(Line::Clock, 16.0, palette::HUD_TEXT),
                ],
            ),
            text(Line::Feedback, 22.0, palette::HUD_TEXT),
            text(Line::Held, 16.0, palette::CHORD_BADGE),
            text(Line::Summary, 32.0, palette::timing(Timing::Perfect)),
            text(Line::Details, 16.0, palette::HUD_TEXT),
            text(Line::WorkOn, 15.0, palette::FIRE),
            (
                Part::Dealing,
                row(10.0),
                children![button(Control::Back, "Stop", 16.0)],
            ),
            (
                Part::Finished,
                row(10.0),
                children![
                    button(Control::StartDrill, "Again", 18.0),
                    button(Control::Back, "Back to the book", 16.0),
                ],
            ),
            text(Line::Hint, 12.0, palette::HUD_DIM),
        ],
    ));
}

/// What the player played instead, in words.
fn mistake(answer: Answer) -> String {
    match answer {
        Answer::Right(_) => String::new(),
        Answer::TimedOut => "Time's up!".to_owned(),
        Answer::Wrong(Mistake::Inversion(inversion)) => {
            format!("Right chord, but that was {}.", inversion.label())
        }
        Answer::Wrong(Mistake::Chord(chord)) => format!("That was {}.", chord.name()),
        Answer::Wrong(Mistake::NotAChord) => {
            "Not quite: that's not a major or minor triad.".to_owned()
        }
    }
}

fn feedback(stage: Stage) -> (String, Color) {
    match stage {
        Stage::Marked {
            answer: Answer::Right(time),
            ..
        } => (
            format!("Right!  {:.1}s", time.0),
            palette::timing(Timing::Good),
        ),
        Stage::Marked { card, answer } => (
            format!(
                "{}  It's {}.",
                mistake(answer),
                card.chord().voicing(card.inversion()).join(" ")
            ),
            palette::MISSED,
        ),
        Stage::Asking { .. } | Stage::Over => (String::new(), palette::HUD_TEXT),
    }
}

/// "2 right  1 wrong  0 too slow".
fn tally(summary: &Summary) -> String {
    format!(
        "{} right  {} wrong  {} too slow",
        summary.right, summary.wrong, summary.timed_out
    )
}

fn summary_lines(summary: &Summary) -> [String; 3] {
    let percent = summary.right * 100 / ROUND;
    let average = summary
        .average
        .map(|t| format!("  |  {:.1}s on average", t.0))
        .unwrap_or_default();
    let work_on = if summary.to_work_on.is_empty() {
        "A clean round!".to_owned()
    } else {
        let cards: Vec<String> = summary
            .to_work_on
            .iter()
            .map(|c| format!("{} ({})", c.chord().name(), c.inversion().label()))
            .collect();
        format!("Work on: {}", cards.join(", "))
    };
    [
        format!("{} of {ROUND} right ({percent}%)", summary.right),
        format!(
            "{} wrong, {} too slow{average}",
            summary.wrong, summary.timed_out
        ),
        work_on,
    ]
}

/// Everything the screen's lines say for this round.
fn line_text(kind: Line, round: &Round, notes: &[MidiNote]) -> String {
    let stage = round.stage();
    let summary = round.summary();
    let over = stage == Stage::Over;
    let card = match stage {
        Stage::Asking { card, .. } | Stage::Marked { card, .. } => Some(card),
        Stage::Over => None,
    };
    let [result, details, work_on] = summary_lines(&summary);
    match kind {
        Line::Header if over => "FLASH CARDS  |  round over".to_owned(),
        Line::Header => format!(
            "FLASH CARDS  |  card {} of {ROUND}  |  {}",
            round.number(),
            tally(&summary)
        ),
        Line::Name => card.map(|c| c.chord().name()).unwrap_or_default(),
        Line::Position => card
            .map(|c| c.inversion().label().to_owned())
            .unwrap_or_default(),
        Line::Clock => format!("{:.1}s", round.left().0),
        Line::Feedback => feedback(stage).0,
        Line::Held => describe_held(notes, is_button_chord(notes))
            .map(|s| s.replace("  (no button)", ""))
            .unwrap_or_default(),
        Line::Summary if over => result,
        Line::Details if over => details,
        Line::WorkOn if over => work_on,
        Line::Summary | Line::Details | Line::WorkOn => String::new(),
        Line::Hint if over => "Enter or Dsus4: again  |  Esc or Bdim: back to the book".to_owned(),
        Line::Hint => format!(
            "Play the chord in that position within {:.0}s  |  Esc or Bdim stops",
            TIME_LIMIT.0
        ),
    }
}

/// From green through gold to red as time runs out.
fn clock_color(left: f32) -> Color {
    let gold = palette::timing(Timing::Perfect);
    if left > 0.5 {
        gold.mix(&palette::timing(Timing::Good), (left - 0.5) * 2.0)
    } else {
        palette::MISSED.mix(&gold, left * 2.0)
    }
}

type Parts<'w, 's> = Query<
    'w,
    's,
    (&'static Part, &'static mut Visibility, &'static mut Node),
    (Without<DrillScreen>, Without<ClockFill>),
>;

#[expect(clippy::too_many_arguments, reason = "Bevy system parameters")]
fn show(
    screen: Res<Screen>,
    cards: Res<FlashCards>,
    held: Res<HeldKeys>,
    mut root: Single<&mut Visibility, With<DrillScreen>>,
    mut parts: Parts,
    mut lines: Query<(&Line, &mut Text, &mut TextColor)>,
    mut clock: Single<(&mut Node, &mut BackgroundColor), With<ClockFill>>,
    mut guide: ResMut<GuideKeys>,
) {
    let open = *screen == Screen::Drill;
    root.set_if_neq(if open {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    });
    let Some(round) = cards.drill.round().filter(|_| open) else {
        return;
    };
    let stage = round.stage();
    // Light the answer once it's been missed, never before.
    let wanted_guide = match stage {
        Stage::Marked { card, answer } if !answer.is_right() => card.keys(),
        _ => Vec::new(),
    };
    if guide.0 != wanted_guide {
        guide.0 = wanted_guide;
    }
    let over = stage == Stage::Over;
    // Hidden so their buttons can't be played, and out of the layout so
    // the rest close up.
    parts
        .iter_mut()
        .for_each(|(part, mut visibility, mut node)| {
            let shown = match part {
                Part::Always => true,
                Part::Dealing => !over,
                Part::Finished => over,
            };
            let (wanted, display) = if shown {
                (Visibility::Inherited, Display::Flex)
            } else {
                (Visibility::Hidden, Display::None)
            };
            visibility.set_if_neq(wanted);
            if node.display != display {
                node.display = display;
            }
        });
    let left = round.left().0 / TIME_LIMIT.0;
    let (node, fill) = &mut *clock;
    node.width = Val::Percent(left * 100.0);
    fill.0 = clock_color(left);
    let notes: Vec<MidiNote> = held.0.iter().copied().collect();
    lines.iter_mut().for_each(|(kind, mut text, mut color)| {
        let wanted = line_text(*kind, round, &notes);
        if text.0 != wanted {
            text.0 = wanted;
        }
        if *kind == Line::Feedback {
            color.set_if_neq(TextColor(feedback(stage).1));
        }
    });
}
