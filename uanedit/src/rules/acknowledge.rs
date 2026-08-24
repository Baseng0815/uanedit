use indexmap::{
    IndexMap,
    IndexSet,
};
use serde::{
    Deserialize,
    Serialize,
};

use crate::rules::fingerprint::Fingerprint;
use crate::types::date_time::DateTime;

/// The findings a file already had when it was opened.
///
/// Anything outside it is this editor's doing, which is what makes "operations never introduce
/// findings" checkable.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Baseline(IndexSet<Fingerprint>);

impl Baseline {
    pub fn contains(
        &self,
        fingerprint: &Fingerprint,
    ) -> bool {
        self.0.contains(fingerprint)
    }

    pub fn insert(
        &mut self,
        fingerprint: Fingerprint,
    ) {
        self.0.insert(fingerprint);
    }

    pub fn iter(&self) -> impl Iterator<Item = &Fingerprint> {
        self.0.iter()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl FromIterator<Fingerprint> for Baseline {
    fn from_iter<T: IntoIterator<Item = Fingerprint>>(fingerprints: T) -> Self {
        Self(fingerprints.into_iter().collect())
    }
}

/// One finding the user has reviewed and chosen to live with — the editor's `#[allow]`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Acknowledgement {
    pub fingerprint: Fingerprint,
    pub reason: Option<String>,
    pub acknowledged_at: Option<DateTime>,
}

impl Acknowledgement {
    pub fn new(fingerprint: Fingerprint) -> Self {
        Self {
            fingerprint,
            reason: None,
            acknowledged_at: None,
        }
    }
}

/// The acknowledgements a workspace holds, which the app persists in a sidecar beside the nodeset.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Acknowledgements(IndexMap<Fingerprint, Acknowledgement>);

impl Acknowledgements {
    pub fn contains(
        &self,
        fingerprint: &Fingerprint,
    ) -> bool {
        self.0.contains_key(fingerprint)
    }

    pub fn get(
        &self,
        fingerprint: &Fingerprint,
    ) -> Option<&Acknowledgement> {
        self.0.get(fingerprint)
    }

    pub fn insert(
        &mut self,
        acknowledgement: Acknowledgement,
    ) -> Option<Acknowledgement> {
        self.0
            .insert(acknowledgement.fingerprint.clone(), acknowledgement)
    }

    pub fn remove(
        &mut self,
        fingerprint: &Fingerprint,
    ) -> Option<Acknowledgement> {
        self.0.shift_remove(fingerprint)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Acknowledgement> {
        self.0.values()
    }

    /// Drops the acknowledgements no current finding matches, which is how one expires.
    pub fn retain_live<'a>(
        &mut self,
        live: impl IntoIterator<Item = &'a Fingerprint>,
    ) {
        let live: IndexSet<&Fingerprint> = live.into_iter().collect();
        self.0.retain(|fingerprint, _| live.contains(fingerprint));
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl FromIterator<Acknowledgement> for Acknowledgements {
    fn from_iter<T: IntoIterator<Item = Acknowledgement>>(entries: T) -> Self {
        Self(
            entries
                .into_iter()
                .map(|entry| (entry.fingerprint.clone(), entry))
                .collect(),
        )
    }
}
