//! What this island has been: how many of each lineage lived here, and
//! how high the sun stood, sampled every few seconds of day.

use std::collections::{BTreeMap, VecDeque};

/// Ticks between samples: two seconds of day, forty of full dark.
pub const EVERY: u64 = 20;
/// Samples kept: eight minutes of day.
pub const LEN: usize = 240;

pub struct Sample {
    pub tick: u64,
    pub sun: u8,
    /// (lineage, motes), biggest first.
    pub counts: Vec<(u64, u32)>,
}

impl Sample {
    pub fn count(&self, lineage: u64) -> u32 {
        self.counts
            .iter()
            .find(|c| c.0 == lineage)
            .map_or(0, |c| c.1)
    }
}

#[derive(Default)]
pub struct History {
    pub samples: VecDeque<Sample>,
    /// Names of the lineages still in some sample.
    names: BTreeMap<u64, String>,
}

impl History {
    /// Note the island as it stands. `lineages` is (lineage, name, motes).
    pub fn note(&mut self, tick: u64, sun: u8, lineages: &[(u64, &str, u32)]) {
        let mut counts: Vec<(u64, u32)> = lineages.iter().map(|l| (l.0, l.2)).collect();
        counts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        for l in lineages {
            self.names.entry(l.0).or_insert_with(|| l.1.to_string());
        }
        self.samples.push_back(Sample { tick, sun, counts });
        if self.samples.len() > LEN {
            if let Some(old) = self.samples.pop_front() {
                for (id, _) in old.counts {
                    if !self.samples.iter().any(|s| s.count(id) > 0) {
                        self.names.remove(&id);
                    }
                }
            }
        }
    }

    pub fn name(&self, lineage: u64) -> &str {
        self.names.get(&lineage).map_or("?", |n| n.as_str())
    }

    /// The lineages worth a line: the person's own that lived here, then
    /// the biggest at their peak, at most `n` in all.
    pub fn series(&self, mine: impl Fn(u64) -> bool, n: usize) -> Vec<u64> {
        let mut peak: BTreeMap<u64, u32> = BTreeMap::new();
        for s in &self.samples {
            for &(id, c) in &s.counts {
                let p = peak.entry(id).or_insert(0);
                *p = (*p).max(c);
            }
        }
        let mut by: Vec<(u64, u32)> = peak.into_iter().collect();
        by.sort_by(|a, b| {
            mine(b.0)
                .cmp(&mine(a.0))
                .then(b.1.cmp(&a.1))
                .then(a.0.cmp(&b.0))
        });
        by.into_iter().take(n).map(|(id, _)| id).collect()
    }

    /// The most motes any of `series` reached, at least one.
    pub fn peak(&self, series: &[u64]) -> u32 {
        self.samples
            .iter()
            .flat_map(|s| series.iter().map(move |&id| s.count(id)))
            .max()
            .unwrap_or(0)
            .max(1)
    }
}
