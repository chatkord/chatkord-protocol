//! Which picture a member sends or another takes: a screen or a camera, each its own track. Both ends and every crate
//! that shows one read it here, so a picture is never a track kind that could be a sound.

use crate::chatkord_v1::TrackKind;

/// A member's picture: at most one of each, each its own stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Video {
    /// A screen or a window, with its sound beside it.
    Screen,
    /// The member's camera.
    Camera,
}

impl Video {
    /// The track it is sent and taken as.
    pub const fn kind(self) -> TrackKind {
        match self {
            Self::Screen => TrackKind::ScreenVideo,
            Self::Camera => TrackKind::Camera,
        }
    }

    /// The picture a track is, none for a sound.
    pub const fn of(kind: TrackKind) -> Option<Video> {
        match kind {
            TrackKind::ScreenVideo => Some(Self::Screen),
            TrackKind::Camera => Some(Self::Camera),
            TrackKind::Microphone | TrackKind::ScreenAudio | TrackKind::Unspecified => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_is_its_track_and_a_sound_is_no_picture() {
        for video in [Video::Screen, Video::Camera] {
            assert_eq!(Video::of(video.kind()), Some(video));
        }
        for kind in [TrackKind::Microphone, TrackKind::ScreenAudio, TrackKind::Unspecified] {
            assert_eq!(Video::of(kind), None, "{kind:?}");
        }
    }
}
