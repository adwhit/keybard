//! The chord book: every major and minor triad, one per page, with its keys
//! lit on the piano and a challenge to play it in all three positions.
//!
//! Its own buttons have sus4 and diminished chords, which no page is, so
//! playing the page's chord is always practice, never a button press. The
//! challenges you clear become flash cards (see [`super::drill`]).

use std::collections::{BTreeSet, HashSet};

use bevy::prelude::*;
use tracing::info;

use super::controls::{Control, ControlButton};
use super::input::HeldKeys;
use super::menu::{Screen, describe_held};
use super::piano::GuideKeys;
use super::widgets::{button, label_text};
use super::{Phase, palette};
use crate::chord_keys::{BOOK, ChordKey, Inversion};
use crate::drill::TIME_LIMIT;
use crate::layout;
use crate::music::MidiNote;
use crate::performance::Timing;

/// Which page is open and which challenges are done (this session).
#[derive(Resource, Debug, Default)]
pub struct ChordBook {
    pub page: usize,
    done: HashSet<(usize, Inversion)>,
    /// What the last success was, to show it.
    news: Option<String>,
}

impl ChordBook {
    /// A book with these challenges already done.
    pub fn with_progress(done: impl IntoIterator<Item = (usize, Inversion)>) -> Self {
        Self {
            done: done
                .into_iter()
                .filter(|(page, _)| *page < BOOK.len())
                .collect(),
            ..Self::default()
        }
    }

    /// The challenges done, for saving.
    pub fn progress(&self) -> BTreeSet<(usize, Inversion)> {
        self.done.iter().copied().collect()
    }

    fn chord(&self) -> ChordKey {
        BOOK[self.page]
    }

    fn has(&self, page: usize, inversion: Inversion) -> bool {
        self.done.contains(&(page, inversion))
    }

    fn page_mastered(&self, page: usize) -> bool {
        Inversion::ALL.iter().all(|i| self.has(page, *i))
    }

    /// Chords played in all three positions.
    pub fn mastered(&self) -> usize {
        (0..BOOK.len()).filter(|p| self.page_mastered(*p)).count()
    }

    /// Show a message on the page until it's turned.
    pub fn tell(&mut self, news: impl Into<String>) {
        self.news = Some(news.into());
    }

    fn turn(&mut self, by: isize) {
        let pages = BOOK.len() as isize;
        self.page = (self.page as isize + by).rem_euclid(pages) as usize;
        self.news = None;
    }
}

#[derive(Component, Debug)]
pub struct ChordBookPage;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Line {
    Page,
    Name,
    Notes,
    Challenges,
    News,
    Held,
    Progress,
    Cards,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<ChordBook>()
        .add_systems(Startup, spawn_book)
        .add_systems(Update, turn_pages.in_set(Phase::Input))
        .add_systems(Update, (practise, show).chain().in_set(Phase::Present));
}

fn line(kind: Line, size: f32, color: Color) -> impl Bundle {
    (
        kind,
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color),
    )
}

fn spawn_book(mut commands: Commands) {
    commands.spawn((
        ChordBookPage,
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
            line(Line::Page, 14.0, palette::HUD_DIM),
            line(Line::Name, 56.0, palette::timing(Timing::Perfect)),
            line(Line::Notes, 18.0, palette::HUD_TEXT),
            label_text("Play it in each position (the keys are lit below):", 14.0),
            line(Line::Challenges, 16.0, palette::HUD_TEXT),
            line(Line::News, 18.0, palette::timing(Timing::Great)),
            line(Line::Held, 16.0, palette::CHORD_BADGE),
            line(Line::Progress, 14.0, palette::FIRE),
            line(Line::Cards, 14.0, palette::HUD_TEXT),
            (
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(12.0),
                    margin: UiRect::top(Val::Px(10.0)),
                    ..default()
                },
                children![
                    button(Control::SeekBack, "< Previous", 16.0),
                    button(Control::SeekForward, "Next >", 16.0),
                    button(Control::StartDrill, "Flash cards", 16.0),
                    // Back, not OpenMenu: OpenMenu's chord is Am, a page.
                    button(Control::Back, "Back", 16.0),
                ],
            ),
            label_text(
                "Play a button's chord to press it  |  Left/Right turn pages  |  Enter: flash cards  |  Esc goes back",
                12.0,
            ),
        ],
    ));
}

fn turn_pages(
    mut controls: MessageReader<Control>,
    screen: Res<Screen>,
    mut book: ResMut<ChordBook>,
) {
    if *screen != Screen::ChordBook {
        return;
    }
    controls.read().for_each(|control| match control {
        Control::SeekBack => book.turn(-1),
        Control::SeekForward => book.turn(1),
        _ => {}
    });
}

/// Tick off a position when the page's chord is played in it.
fn practise(screen: Res<Screen>, held: Res<HeldKeys>, mut book: ResMut<ChordBook>) {
    if *screen != Screen::ChordBook {
        return;
    }
    let notes: Vec<MidiNote> = held.0.iter().copied().collect();
    let chord = book.chord();
    let Some(inversion) = chord.inversion(&notes) else {
        return;
    };
    let page = book.page;
    if book.done.insert((page, inversion)) {
        info!(
            chord = chord.label(),
            ?inversion,
            "chord book challenge done"
        );
        book.news = Some(if book.page_mastered(page) {
            format!("{} mastered!", chord.name())
        } else {
            format!("Got it: {}", inversion.label())
        });
    }
}

/// What the flash-cards button will deal.
fn cards_line(cleared: usize) -> String {
    match cleared {
        0 => "Flash cards: Clear a challenge to make your first card.".to_owned(),
        1 => format!("Flash cards: 1 card ready, {:.0}s to play it", TIME_LIMIT.0),
        n => format!(
            "Flash cards: {n} cards ready, {:.0}s to play each",
            TIME_LIMIT.0
        ),
    }
}

pub(super) fn show(
    screen: Res<Screen>,
    book: Res<ChordBook>,
    held: Res<HeldKeys>,
    mut root: Single<&mut Visibility, With<ChordBookPage>>,
    mut lines: Query<(&Line, &mut Text)>,
    mut guide: ResMut<GuideKeys>,
    buttons: Query<(&ControlButton, &InheritedVisibility)>,
) {
    let open = *screen == Screen::ChordBook;
    root.set_if_neq(if open {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    });
    // The flash cards light (or hide) the keys themselves.
    if *screen != Screen::Drill {
        let wanted_guide = if open {
            book.chord().keys(Inversion::Root)
        } else {
            Vec::new()
        };
        if guide.0 != wanted_guide {
            guide.0 = wanted_guide;
        }
    }
    if !open {
        return;
    }
    let chord = book.chord();
    let notes: Vec<MidiNote> = held.0.iter().copied().collect();
    lines.iter_mut().for_each(|(kind, mut text)| {
        let wanted = match kind {
            Line::Page => format!("CHORD BOOK  |  page {} of {}", book.page + 1, BOOK.len()),
            Line::Name => chord.name(),
            Line::Notes => {
                let [root, third, fifth] = chord.note_names();
                format!("{root} {third} {fifth}  |  root {root}, third {third}, fifth {fifth}")
            }
            Line::Challenges => Inversion::ALL
                .iter()
                .map(|inversion| {
                    let mark = if book.has(book.page, *inversion) {
                        "x"
                    } else {
                        " "
                    };
                    format!(
                        "[{mark}] {}  ({})",
                        inversion.label(),
                        chord.voicing(*inversion).join(" ")
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
            Line::News => book.news.clone().unwrap_or_default(),
            Line::Held => {
                let button_chord = buttons
                    .iter()
                    .filter(|(_, visible)| visible.get())
                    .any(|(b, _)| b.0.chord().is_some_and(|c| c.matches(&notes)));
                describe_held(&notes, button_chord)
                    .map(|s| s.replace("  (no button)", ""))
                    .unwrap_or_default()
            }
            Line::Progress => format!("Chords mastered: {}/{}", book.mastered(), BOOK.len()),
            Line::Cards => cards_line(book.done.len()),
        };
        if text.0 != wanted {
            text.0 = wanted;
        }
    });
}
