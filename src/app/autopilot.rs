//! A built-in player that presses the right keys with a little human-like
//! timing jitter. Useful for demos, snapshots and end-to-end tests.

use std::collections::HashSet;

use bevy::prelude::*;

use super::input::{InputSource, KeyEdge, NoteInput};
use super::transport::{PlayState, Transport};
use super::{ActiveSong, Phase};
use crate::performance::PlayMode;
use crate::song::{NoteId, Seconds};

/// Maximum timing error either side of the beat.
const MAX_JITTER: f32 = 0.07;

#[derive(Resource, Debug, Default)]
struct Hands {
    down: HashSet<NoteId>,
    done: HashSet<NoteId>,
}

/// Deterministic per-note timing error in `-MAX_JITTER..MAX_JITTER`.
pub fn jitter(id: NoteId) -> Seconds {
    let hash = (id.0 as u32).wrapping_mul(2_654_435_761) >> 16;
    let unit = (hash % 1000) as f32 / 999.0;
    Seconds((unit * 2.0 - 1.0) * MAX_JITTER)
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Hands>()
        .add_systems(Update, play.in_set(Phase::Input));
}

fn play(
    mut hands: ResMut<Hands>,
    transport: Res<Transport>,
    song: Res<ActiveSong>,
    mut out: MessageWriter<NoteInput>,
) {
    if !transport.mode.is_scored() || transport.state != PlayState::Playing {
        return;
    }
    let now = transport.time;
    // Forget everything when the clock goes back (restart / seek).
    if hands
        .done
        .iter()
        .any(|id| song.0.note(*id).start > now + Seconds(MAX_JITTER))
    {
        *hands = Hands::default();
    }
    let releases: Vec<NoteId> = hands
        .down
        .iter()
        .copied()
        .filter(|id| song.0.note(*id).end() <= now)
        .collect();
    releases.iter().for_each(|id| {
        hands.down.remove(id);
        out.write(NoteInput {
            note: song.0.note(*id).pitch,
            edge: KeyEdge::Up,
            source: InputSource::Autopilot,
        });
    });
    let presses: Vec<NoteId> = song
        .0
        .iter()
        .take_while(|(_, n)| n.start <= now + Seconds(MAX_JITTER))
        .filter(|(id, n)| {
            let due = n.start + jitter(*id);
            // In Wait mode the clock parks at the note, so don't wait for lateness.
            let ready = now >= due || (transport.mode == PlayMode::Wait && now >= n.start);
            ready && !hands.done.contains(id) && n.end() > now
        })
        .map(|(id, _)| id)
        .collect();
    presses.iter().for_each(|id| {
        hands.done.insert(*id);
        hands.down.insert(*id);
        out.write(NoteInput {
            note: song.0.note(*id).pitch,
            edge: KeyEdge::Down,
            source: InputSource::Autopilot,
        });
    });
}
