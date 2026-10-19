use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub version: u8,
    pub warmup_seconds: u32,
    pub cooldown_seconds: u32,
    pub hourly_limit: usize,
    pub rules: Vec<Rule>,
    #[serde(default)]
    pub semantic: Semantic,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Semantic {
    pub enabled: bool,
    pub stories: bool,
    pub visual: bool,
    pub interval_seconds: u32,
    pub min_confidence: f64,
    pub guidance: String,
    pub exclusions: String,
    pub context_seconds: u32,
    pub language: String,
    pub use_feedback: bool,
    pub rules: Vec<ContentRule>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentRule {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub kind: String,
    pub description: String,
}

impl Default for Semantic {
    fn default() -> Self {
        Self {
            enabled: false,
            stories: true,
            visual: true,
            interval_seconds: 90,
            min_confidence: 0.8,
            guidance: String::new(),
            exclusions: String::new(),
            context_seconds: 120,
            language: "auto".into(),
            use_feedback: true,
            rules: Vec::new(),
        }
    }
}

impl Semantic {
    pub fn needs_transcript(&self) -> bool {
        self.stories || self.rules.iter().any(|r| r.enabled && r.kind == "story")
    }

    pub fn accepts(&self, kind: &str, trigger_id: &str) -> bool {
        match trigger_id {
            "story" | "" if kind == "story" => self.stories,
            "visual" | "" if kind == "visual" => self.visual,
            id => self
                .rules
                .iter()
                .any(|r| r.enabled && r.id == id && r.kind == kind),
        }
    }

    pub fn criteria(&self) -> Vec<ContentRule> {
        let mut rules = self
            .rules
            .iter()
            .filter(|r| r.enabled)
            .cloned()
            .collect::<Vec<_>>();
        if self.stories {
            rules.insert(0, ContentRule { id: "story".into(), name: "Stories & Gespräche".into(), enabled: true, kind: "story".into(), description: "Eigenständige Anekdoten, absurde Monologe, Pointen und interessante Gespräche mit Aufbau und Abschluss.".into() });
        }
        if self.visual {
            rules.push(ContentRule {
                id: "visual".into(),
                name: "Video-Momente".into(),
                enabled: true,
                kind: "visual".into(),
                description:
                    "Belegte sichtbare Überraschungen, Reaktionen, Fails und körperliche Comedy."
                        .into(),
            });
        }
        rules
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub name: String,
    pub enabled: bool,
    pub hold_seconds: f64,
    pub conditions: Vec<Condition>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Condition {
    pub metric: Metric,
    pub op: Operator,
    pub value: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Metric {
    AudioRise,
    AudioLevel,
    ChatMessages,
    ChatUsers,
    ChatRate,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operator {
    Gte,
    Lte,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            version: 1,
            warmup_seconds: 60,
            cooldown_seconds: 120,
            hourly_limit: 6,
            semantic: Semantic::default(),
            rules: vec![Rule {
                name: "Reaktion mit Chat".into(),
                enabled: true,
                hold_seconds: 1.5,
                conditions: vec![
                    Condition {
                        metric: Metric::AudioRise,
                        op: Operator::Gte,
                        value: 8.0,
                    },
                    Condition {
                        metric: Metric::AudioLevel,
                        op: Operator::Gte,
                        value: -24.0,
                    },
                    Condition {
                        metric: Metric::ChatMessages,
                        op: Operator::Gte,
                        value: 5.0,
                    },
                    Condition {
                        metric: Metric::ChatUsers,
                        op: Operator::Gte,
                        value: 3.0,
                    },
                ],
            }],
        }
    }
}

impl Profile {
    pub fn validate(&self) -> Result<(), String> {
        if !(30..=300).contains(&self.semantic.interval_seconds)
            || !(0.5..=1.0).contains(&self.semantic.min_confidence)
            || self.semantic.guidance.chars().count() > 1000
            || self.semantic.exclusions.chars().count() > 1000
            || !(60..=180).contains(&self.semantic.context_seconds)
            || !["auto", "de", "en"].contains(&self.semantic.language.as_str())
            || (self.semantic.enabled && self.semantic.criteria().is_empty())
        {
            return Err("Inhaltliche Trigger: mindestens ein aktiver Auslöser, Intervall 30–300s, Kontext 60–180s, Bewertung 50–100% und maximal 1000 Zeichen je Beschreibung.".into());
        }
        let mut ids = std::collections::HashSet::new();
        if self.semantic.rules.len() > 12
            || self.semantic.rules.iter().any(|r| {
                r.id.is_empty()
                    || r.id.len() > 64
                    || !r
                        .id
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                    || ["story", "visual", "none"].contains(&r.id.as_str())
                    || !ids.insert(&r.id)
                    || r.name.trim().is_empty()
                    || r.name.chars().count() > 60
                    || r.description.trim().is_empty()
                    || r.description.chars().count() > 1000
                    || !["story", "visual"].contains(&r.kind.as_str())
            })
        {
            return Err("Bis zu 12 Text-Trigger mit eindeutiger ID, Name (1–60 Zeichen), Kategorie story/visual und Beschreibung (1–1000 Zeichen) sind möglich.".into());
        }
        if self.version != 1
            || !(15..=180).contains(&self.warmup_seconds)
            || !(30..=1800).contains(&self.cooldown_seconds)
            || !(1..=60).contains(&self.hourly_limit)
            || !(1..=8).contains(&self.rules.len())
        {
            return Err("Trigger: 1–8 Regeln, Kalibrierung 15–180s, Abstand 30–1800s und 1–60 Clips/Stunde erforderlich.".into());
        }
        for rule in &self.rules {
            if rule.name.trim().is_empty()
                || rule.name.chars().count() > 60
                || !(0.5..=10.0).contains(&rule.hold_seconds)
                || !(1..=6).contains(&rule.conditions.len())
            {
                return Err("Jede Regel benötigt einen Namen (max. 60 Zeichen), 1–6 Bedingungen und 0,5–10s Mindestdauer.".into());
            }
            for condition in &rule.conditions {
                let (min, max) = match condition.metric {
                    Metric::AudioRise => (3.0, 30.0),
                    Metric::AudioLevel => (-60.0, 0.0),
                    Metric::ChatMessages | Metric::ChatUsers => (1.0, 10000.0),
                    Metric::ChatRate => (1.0, 30.0),
                };
                if !(min..=max).contains(&condition.value) {
                    return Err(
                        "Eine Trigger-Schwelle liegt außerhalb des erlaubten Bereichs.".into(),
                    );
                }
            }
        }
        Ok(())
    }

    pub fn uses_chat(&self) -> bool {
        self.rules
            .iter()
            .filter(|r| r.enabled)
            .flat_map(|r| &r.conditions)
            .any(|c| {
                matches!(
                    c.metric,
                    Metric::ChatMessages | Metric::ChatUsers | Metric::ChatRate
                )
            })
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Signal {
    pub time_ms: i64,
    pub audio_level: f64,
    pub audio_rise: f64,
    pub baseline_db: f64,
    pub calibrated_seconds: f64,
    pub chat_messages: Option<f64>,
    pub chat_users: Option<f64>,
    pub chat_rate: Option<f64>,
}

impl Signal {
    pub fn value(&self, metric: Metric) -> Option<f64> {
        match metric {
            Metric::AudioRise => Some(self.audio_rise),
            Metric::AudioLevel => Some(self.audio_level),
            Metric::ChatMessages => self.chat_messages,
            Metric::ChatUsers => self.chat_users,
            Metric::ChatRate => self.chat_rate,
        }
    }
}

impl Condition {
    pub fn matches(&self, signal: &Signal) -> bool {
        signal
            .value(self.metric)
            .is_some_and(|value| match self.op {
                Operator::Gte => value >= self.value,
                Operator::Lte => value <= self.value,
            })
    }

    fn describe(&self, signal: &Signal) -> String {
        let label = match self.metric {
            Metric::AudioRise => "Audioanstieg (dB)",
            Metric::AudioLevel => "Audiopegel (dBFS)",
            Metric::ChatMessages => "Chatnachrichten/10s",
            Metric::ChatUsers => "Chatter/10s",
            Metric::ChatRate => "Chat-Faktor",
        };
        format!(
            "{label}: {:.1} {} {:.1}",
            signal.value(self.metric).unwrap_or(0.0),
            if self.op == Operator::Gte {
                "≥"
            } else {
                "≤"
            },
            self.value
        )
    }
}

#[derive(Default)]
pub struct Analyzer {
    history: VecDeque<f64>,
    samples: usize,
    last_ms: Option<i64>,
}

impl Analyzer {
    pub fn feed(&mut self, pcm: &[i16], start_ms: i64) -> Vec<Signal> {
        let mut frames = Vec::new();
        for (index, window) in pcm.as_chunks::<4000>().0.iter().enumerate() {
            let time_ms = start_ms + index as i64 * 250;
            if self
                .last_ms
                .is_some_and(|last| time_ms - last > 1000 || time_ms <= last)
            {
                self.history.clear();
                self.samples = 0;
            }
            self.last_ms = Some(time_ms);
            let rms = (window
                .iter()
                .map(|&s| (f64::from(s) / 32768.0).powi(2))
                .sum::<f64>()
                / 4000.0)
                .sqrt();
            let level = (20.0 * rms.max(1e-8).log10()).max(-96.0);
            // Upper-quartile active audio avoids treating every sentence after silence as a reaction.
            let mut active: Vec<f64> = self
                .history
                .iter()
                .copied()
                .filter(|v| *v > -55.0)
                .collect();
            active.sort_by(f64::total_cmp);
            let baseline = active
                .get(active.len() * 3 / 4)
                .copied()
                .unwrap_or(level)
                .max(-45.0);
            self.samples += 1;
            frames.push(Signal {
                time_ms,
                audio_level: level,
                audio_rise: level - baseline,
                baseline_db: baseline,
                calibrated_seconds: self.samples as f64 / 4.0,
                ..Signal::default()
            });
            self.history.push_back(level);
            if self.history.len() > 480 {
                self.history.pop_front();
            }
        }
        frames
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Hit {
    pub time_ms: i64,
    pub rule: String,
    pub reason: String,
}

#[derive(Default)]
pub struct Engine {
    profile: Option<Profile>,
    holds: Vec<u32>,
    latched: Vec<bool>,
    previous_ms: Option<i64>,
    recent: VecDeque<i64>,
}

impl Engine {
    pub fn evaluate(&mut self, profile: &Profile, signal: &Signal) -> Option<Hit> {
        if self.profile.as_ref() != Some(profile) {
            self.profile = Some(profile.clone());
            self.holds = vec![0; profile.rules.len()];
            self.latched = vec![false; profile.rules.len()];
        }
        if self
            .previous_ms
            .is_some_and(|last| signal.time_ms - last > 500 || signal.time_ms <= last)
        {
            self.holds.fill(0);
            self.latched.fill(false);
        }
        self.previous_ms = Some(signal.time_ms);
        while self
            .recent
            .front()
            .is_some_and(|time| signal.time_ms - time >= 3_600_000)
        {
            self.recent.pop_front();
        }
        let ready = signal.calibrated_seconds >= f64::from(profile.warmup_seconds);
        let allowed = self.recent.len() < profile.hourly_limit
            && self.recent.back().is_none_or(|time| {
                signal.time_ms - time >= i64::from(profile.cooldown_seconds) * 1000
            });
        let mut hit = None;
        for (index, rule) in profile.rules.iter().enumerate() {
            let matches =
                ready && rule.enabled && rule.conditions.iter().all(|c| c.matches(signal));
            if !matches {
                self.holds[index] = 0;
                self.latched[index] = false;
                continue;
            }
            self.holds[index] = self.holds[index].saturating_add(250);
            if f64::from(self.holds[index]) >= rule.hold_seconds * 1000.0 && !self.latched[index] {
                self.latched[index] = true;
                if !allowed || hit.is_some() {
                    continue;
                }
                hit = Some(Hit {
                    time_ms: signal.time_ms,
                    rule: rule.name.clone(),
                    reason: format!(
                        "{} · {:.2}s bestätigt · {}",
                        rule.name,
                        rule.hold_seconds,
                        rule.conditions
                            .iter()
                            .map(|c| c.describe(signal))
                            .collect::<Vec<_>>()
                            .join("; ")
                    ),
                });
            }
        }
        if hit.is_some() {
            self.recent.push_back(signal.time_ms);
        }
        hit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signal(time_ms: i64) -> Signal {
        Signal {
            time_ms,
            audio_level: -15.0,
            audio_rise: 12.0,
            calibrated_seconds: 120.0,
            chat_messages: Some(8.0),
            chat_users: Some(4.0),
            ..Signal::default()
        }
    }
    #[test]
    fn requires_all_signals_duration_and_available_chat() {
        let profile = Profile::default();
        for missing in [true, false] {
            let mut engine = Engine::default();
            for t in (0..5000).step_by(250) {
                let mut s = signal(t);
                if missing {
                    s.chat_users = None;
                } else {
                    s.chat_users = Some(1.0);
                }
                assert!(engine.evaluate(&profile, &s).is_none());
            }
        }
        let mut engine = Engine::default();
        for t in (0..1250).step_by(250) {
            assert!(engine.evaluate(&profile, &signal(t)).is_none());
        }
        assert!(engine.evaluate(&profile, &signal(1250)).is_some());
        for t in (1500..180000).step_by(250) {
            assert!(engine.evaluate(&profile, &signal(t)).is_none());
        }
    }
    #[test]
    fn respects_warmup_gaps_cooldown_and_hourly_budget_across_edits() {
        let mut profile = Profile {
            hourly_limit: 2,
            cooldown_seconds: 30,
            ..Profile::default()
        };
        profile.rules[0].hold_seconds = 0.5;
        let mut engine = Engine::default();
        let mut cold = signal(0);
        cold.calibrated_seconds = 1.0;
        assert!(engine.evaluate(&profile, &cold).is_none());
        assert!(engine.evaluate(&profile, &signal(250)).is_none());
        assert!(engine.evaluate(&profile, &signal(2000)).is_none());
        assert!(engine.evaluate(&profile, &signal(2250)).is_some());
        assert!(engine.evaluate(&profile, &signal(4000)).is_none());
        assert!(engine.evaluate(&profile, &signal(4250)).is_none());
        assert!(engine.evaluate(&profile, &signal(33000)).is_none());
        assert!(engine.evaluate(&profile, &signal(33250)).is_some());
        profile.rules[0].name = "Edited".into();
        assert!(engine.evaluate(&profile, &signal(70000)).is_none());
        assert!(engine.evaluate(&profile, &signal(70250)).is_none());
    }
    #[test]
    fn alternative_rules_work_and_invalid_profiles_are_rejected() {
        let mut profile = Profile::default();
        let mut alternative = profile.rules[0].clone();
        alternative
            .conditions
            .retain(|c| c.metric == Metric::AudioRise);
        alternative.hold_seconds = 0.5;
        profile.rules.push(alternative);
        let mut engine = Engine::default();
        let mut s = signal(0);
        s.chat_users = None;
        assert!(engine.evaluate(&profile, &s).is_none());
        s.time_ms = 250;
        assert!(engine.evaluate(&profile, &s).is_some());
        profile.rules[0].conditions.clear();
        assert!(profile.validate().is_err());
        profile = Profile::default();
        profile.rules[0].hold_seconds = f64::NAN;
        assert!(profile.validate().is_err());
    }
    #[test]
    fn silence_and_normal_speech_do_not_make_a_low_baseline() {
        let mut analyzer = Analyzer::default();
        let mut pcm = Vec::new();
        for i in 0..240 {
            pcm.extend(vec![if i % 4 == 0 { 4000 } else { 0 }; 4000]);
        }
        analyzer.feed(&pcm, 0);
        let frames = analyzer.feed(&vec![5000; 32000], 60000);
        assert!(frames.iter().all(|s| s.audio_rise < 3.0));
    }

    #[test]
    fn audio_only_ignores_short_impulses_and_constant_loudness() {
        let mut profile = Profile::default();
        profile.rules[0].hold_seconds = 2.0;
        profile.rules[0]
            .conditions
            .retain(|c| matches!(c.metric, Metric::AudioRise | Metric::AudioLevel));
        for amplitude in [0, 20000] {
            let mut analyzer = Analyzer::default();
            let mut engine = Engine::default();
            assert!(
                analyzer
                    .feed(&vec![amplitude; 16000 * 90], 0)
                    .iter()
                    .all(|s| engine.evaluate(&profile, s).is_none())
            );
        }
        let mut analyzer = Analyzer::default();
        let mut engine = Engine::default();
        for s in analyzer.feed(&vec![1000; 16000 * 65], 0) {
            assert!(engine.evaluate(&profile, &s).is_none());
        }
        let mut impulse = vec![20000; 4000];
        impulse.extend(vec![1000; 12000]);
        for s in analyzer.feed(&impulse, 65000) {
            assert!(engine.evaluate(&profile, &s).is_none());
        }
        let hits: Vec<_> = analyzer
            .feed(&vec![20000; 16000 * 3], 66000)
            .iter()
            .filter_map(|s| engine.evaluate(&profile, s))
            .collect();
        assert_eq!(hits.len(), 1);
    }
}
