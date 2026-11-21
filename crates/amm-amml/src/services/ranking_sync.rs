use amml_api::{ApiError, Input};

/// Gives the loader the input lease policy of the operator's `Input` resolutions in the panel: the ranking, highest
/// first, and the pairs set to `Coexist`. The manager pushes both in `enable`, so it owns the policy (the first mod to
/// set either owns both), and again when the panel changes them. A push the loader refused is not tried again until
/// the value changes.
#[derive(Debug, Default)]
pub struct RankingSync {
    last: Option<Vec<String>>,
    last_coexist: Option<Vec<(String, String)>>,
}

impl RankingSync {
    pub fn new() -> Self {
        Self { last: None, last_coexist: None }
    }

    /// Sets `pairs` as the loader's coexist pairs. `Refused` when another mod owns the policy, `NotAvailable` on a
    /// loader without coexist pairs.
    pub fn push_coexist(&mut self, input: &Input, pairs: &[(String, String)]) -> Result<(), ApiError> {
        self.last_coexist = Some(pairs.to_vec());
        let ids: Vec<(&str, &str)> = pairs.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        input.set_coexist(&ids)
    }

    /// The pairs of the last coexist push, whether the loader took them or not; `None` before the first push.
    pub fn last_coexist(&self) -> Option<&[(String, String)]> {
        self.last_coexist.as_deref()
    }

    /// Sets `ranking` as the loader's lease ranking. `Refused` when another mod owns the list.
    pub fn push(&mut self, input: &Input, ranking: &[String]) -> Result<(), ApiError> {
        self.last = Some(ranking.to_vec());
        let ids: Vec<&str> = ranking.iter().map(String::as_str).collect();
        input.set_ranking(&ids)
    }

    /// The ranking of the last push, whether the loader took it or not; `None` before the first push.
    pub fn last(&self) -> Option<&[String]> {
        self.last.as_deref()
    }
}
