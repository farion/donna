use crate::config::AttentionConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AttentionLevel {
    Info,
    Normal,
    Important,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttentionCandidate {
    pub level: AttentionLevel,
    pub snoozed_until: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttentionDecision {
    pub notify: bool,
    pub raise_window: bool,
    pub use_attention_avatar: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttentionPolicy {
    enabled: bool,
    notification_min_level: AttentionLevel,
    popup_min_level: AttentionLevel,
    popup_cooldown_seconds: i64,
    critical_bypasses_cooldown: bool,
}

impl AttentionPolicy {
    pub fn from_config(config: &AttentionConfig) -> Self {
        Self {
            enabled: config.enabled,
            notification_min_level: AttentionLevel::from_name(&config.notification_min_level)
                .unwrap_or(AttentionLevel::Normal),
            popup_min_level: AttentionLevel::from_name(&config.popup_min_level)
                .unwrap_or(AttentionLevel::Important),
            popup_cooldown_seconds: i64::from(config.popup_cooldown_seconds),
            critical_bypasses_cooldown: config.critical_bypasses_cooldown,
        }
    }

    pub fn decide(
        &self,
        candidate: AttentionCandidate,
        now: i64,
        last_popup_at: Option<i64>,
    ) -> AttentionDecision {
        if !self.enabled || candidate.snoozed_until.is_some_and(|until| until > now) {
            return AttentionDecision::quiet();
        }

        let notify = candidate.level >= self.notification_min_level;
        let past_cooldown = last_popup_at
            .map(|last| now.saturating_sub(last) >= self.popup_cooldown_seconds)
            .unwrap_or(true);
        let bypass_cooldown =
            self.critical_bypasses_cooldown && matches!(candidate.level, AttentionLevel::Critical);
        let raise_window =
            candidate.level >= self.popup_min_level && (past_cooldown || bypass_cooldown);

        AttentionDecision {
            notify,
            raise_window,
            use_attention_avatar: notify || raise_window,
        }
    }
}

impl AttentionLevel {
    pub fn from_name(value: &str) -> Option<Self> {
        match value {
            "info" => Some(Self::Info),
            "normal" => Some(Self::Normal),
            "important" => Some(Self::Important),
            "critical" => Some(Self::Critical),
            _ => None,
        }
    }
}

impl AttentionDecision {
    fn quiet() -> Self {
        Self {
            notify: false,
            raise_window: false,
            use_attention_avatar: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn important_attention_raises_window_and_uses_attention_avatar() {
        let policy = AttentionPolicy::from_config(&AttentionConfig::default());

        let decision = policy.decide(
            AttentionCandidate {
                level: AttentionLevel::Important,
                snoozed_until: None,
            },
            1_000,
            None,
        );

        assert!(decision.notify);
        assert!(decision.raise_window);
        assert!(decision.use_attention_avatar);
    }

    #[test]
    fn normal_attention_notifies_without_popup() {
        let policy = AttentionPolicy::from_config(&AttentionConfig::default());

        let decision = policy.decide(
            AttentionCandidate {
                level: AttentionLevel::Normal,
                snoozed_until: None,
            },
            1_000,
            None,
        );

        assert!(decision.notify);
        assert!(!decision.raise_window);
    }

    #[test]
    fn snoozed_attention_stays_quiet_until_due() {
        let policy = AttentionPolicy::from_config(&AttentionConfig::default());

        let quiet = policy.decide(
            AttentionCandidate {
                level: AttentionLevel::Critical,
                snoozed_until: Some(2_000),
            },
            1_000,
            Some(900),
        );
        let due = policy.decide(
            AttentionCandidate {
                level: AttentionLevel::Critical,
                snoozed_until: Some(2_000),
            },
            2_000,
            Some(1_950),
        );

        assert!(!quiet.notify);
        assert!(due.raise_window);
    }
}
