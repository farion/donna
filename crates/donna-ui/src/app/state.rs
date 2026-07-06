use crate::avatar::AvatarState;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DonnaState {
    Idle,
    Hidden,
    Thinking,
    Attention,
    Question,
    Command,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct AvatarSignals {
    pub command_mode: bool,
    pub hidden: bool,
    pub active_response: bool,
    pub active_question: bool,
    pub active_attention: bool,
}

pub(super) fn resolve_state(signals: AvatarSignals) -> DonnaState {
    if signals.command_mode {
        DonnaState::Command
    } else if signals.active_question {
        DonnaState::Question
    } else if signals.active_attention {
        DonnaState::Attention
    } else if signals.hidden {
        DonnaState::Hidden
    } else if signals.active_response {
        DonnaState::Thinking
    } else {
        DonnaState::Idle
    }
}

pub(super) fn random_idle_frame() -> u8 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.subsec_nanos())
        .unwrap_or(0);
    ((nanos % 3) + 1) as u8
}

impl DonnaState {
    pub(super) fn avatar_state(self, idle_frame: u8) -> AvatarState {
        match self {
            DonnaState::Idle if idle_frame == 0 => AvatarState::Default,
            DonnaState::Idle => AvatarState::Idle(idle_frame),
            DonnaState::Hidden | DonnaState::Attention => AvatarState::Attention,
            DonnaState::Thinking => AvatarState::Thinking,
            DonnaState::Question => AvatarState::Question,
            DonnaState::Command => AvatarState::Command,
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            DonnaState::Idle => "Idle",
            DonnaState::Hidden => "Hidden",
            DonnaState::Thinking => "Thinking",
            DonnaState::Attention => "Attention",
            DonnaState::Question => "Question",
            DonnaState::Command => "Command",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AvatarSignals, DonnaState, resolve_state};
    use crate::avatar::AvatarState;

    #[test]
    fn command_mode_has_top_priority() {
        let state = resolve_state(AvatarSignals {
            command_mode: true,
            active_question: true,
            active_attention: true,
            active_response: true,
            hidden: true,
        });

        assert_eq!(state, DonnaState::Command);
    }

    #[test]
    fn idle_uses_default_avatar_until_pulse_frame() {
        assert_eq!(DonnaState::Idle.avatar_state(0), AvatarState::Default);
        assert_eq!(DonnaState::Idle.avatar_state(2), AvatarState::Idle(2));
    }

    #[test]
    fn question_attention_hidden_and_thinking_are_prioritized() {
        assert_eq!(
            resolve_state(AvatarSignals {
                active_question: true,
                active_attention: true,
                hidden: true,
                active_response: true,
                ..AvatarSignals::default()
            }),
            DonnaState::Question
        );
        assert_eq!(
            resolve_state(AvatarSignals {
                active_attention: true,
                hidden: true,
                active_response: true,
                ..AvatarSignals::default()
            }),
            DonnaState::Attention
        );
        assert_eq!(
            resolve_state(AvatarSignals {
                hidden: true,
                active_response: true,
                ..AvatarSignals::default()
            }),
            DonnaState::Hidden
        );
        assert_eq!(
            resolve_state(AvatarSignals {
                active_response: true,
                ..AvatarSignals::default()
            }),
            DonnaState::Thinking
        );
    }
}
