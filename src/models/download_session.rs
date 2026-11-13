#[derive(Clone, Debug)]
pub struct MediaItem {
    pub title: String,
    pub url: String,
    pub channel: String,
    pub estimated_bytes: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct DownloadPlan {
    pub source_url: String,
    pub batch_name: String,
    pub playlist_name: Option<String>,
    pub items: Vec<MediaItem>,
    pub estimated_bytes: u64,
    pub unknown_size_count: usize,
    pub free_bytes: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DownloadPhase {
    #[default]
    Idle,
    Inspecting,
    PreviewReady,
    Downloading,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DownloadItemStatus {
    Ready,
    Attempt { attempt: u8, total_attempts: u8 },
    Verifying,
    Completed { size_bytes: u64 },
    Aborting,
    Aborted,
    Failed { message: String, retrying: bool },
}

#[derive(Clone, Debug, PartialEq)]
pub struct DownloadItemProgress {
    pub title: String,
    pub percent: f32,
    pub weight: f64,
    pub status: DownloadItemStatus,
    pub can_abort: bool,
}

#[derive(Clone, Debug, Default)]
pub struct DownloadSession {
    phase: DownloadPhase,
    plan: Option<DownloadPlan>,
    items: Vec<DownloadItemProgress>,
    auto_download_after_inspection: bool,
}

impl DownloadSession {
    pub fn begin_inspection(&mut self, auto_download: bool) {
        self.phase = DownloadPhase::Inspecting;
        self.plan = None;
        self.items.clear();
        self.auto_download_after_inspection = auto_download;
    }

    pub fn finish_inspection(&mut self) -> bool {
        self.phase = DownloadPhase::Idle;
        std::mem::take(&mut self.auto_download_after_inspection)
    }

    pub fn set_plan(&mut self, plan: DownloadPlan) {
        self.items = initial_progress(&plan.items);
        self.plan = Some(plan);
        self.phase = DownloadPhase::PreviewReady;
    }

    pub fn begin_download(&mut self) -> Option<DownloadPlan> {
        let plan = self.plan.clone()?;
        self.items = initial_progress(&plan.items);
        self.phase = DownloadPhase::Downloading;
        Some(plan)
    }

    pub fn finish_download(&mut self) {
        self.phase = DownloadPhase::Idle;
    }

    pub fn clear(&mut self) {
        self.phase = DownloadPhase::Idle;
        self.plan = None;
        self.items.clear();
        self.auto_download_after_inspection = false;
    }

    pub fn cancel_auto_download(&mut self) {
        self.auto_download_after_inspection = false;
    }

    pub fn is_inspecting(&self) -> bool {
        self.phase == DownloadPhase::Inspecting
    }

    pub fn is_downloading(&self) -> bool {
        self.phase == DownloadPhase::Downloading
    }

    pub fn is_busy(&self) -> bool {
        matches!(
            self.phase,
            DownloadPhase::Inspecting | DownloadPhase::Downloading
        )
    }

    pub fn plan(&self) -> Option<&DownloadPlan> {
        self.plan.as_ref()
    }

    pub fn items(&self) -> &[DownloadItemProgress] {
        &self.items
    }

    pub fn has_progress(&self) -> bool {
        !self.items.is_empty()
    }

    pub fn overall_progress(&self) -> f32 {
        let total_weight = self.items.iter().map(|item| item.weight).sum::<f64>();
        if total_weight == 0.0 {
            return 0.0;
        }
        (self
            .items
            .iter()
            .map(|item| item.percent as f64 * item.weight)
            .sum::<f64>()
            / total_weight) as f32
    }

    pub fn failed_count(&self) -> usize {
        self.items
            .iter()
            .filter(|item| {
                matches!(
                    item.status,
                    DownloadItemStatus::Failed {
                        retrying: false,
                        ..
                    }
                )
            })
            .count()
    }

    pub fn aborted_count(&self) -> usize {
        self.items
            .iter()
            .filter(|item| item.status == DownloadItemStatus::Aborted)
            .count()
    }

    pub fn mark_attempt_started(&mut self, index: usize, attempt: u8, total_attempts: u8) {
        if let Some(item) = self.items.get_mut(index) {
            item.percent = 0.0;
            item.can_abort = true;
            item.status = DownloadItemStatus::Attempt {
                attempt,
                total_attempts,
            };
        }
    }

    pub fn report_progress(&mut self, index: usize, percent: f32) {
        if let Some(item) = self.items.get_mut(index) {
            item.percent = percent;
        }
    }

    pub fn mark_verifying(&mut self, index: usize) {
        if let Some(item) = self.items.get_mut(index) {
            item.can_abort = false;
            item.status = DownloadItemStatus::Verifying;
        }
    }

    pub fn mark_completed(&mut self, index: usize, size_bytes: u64) {
        if let Some(item) = self.items.get_mut(index) {
            item.percent = 100.0;
            item.can_abort = false;
            item.status = DownloadItemStatus::Completed { size_bytes };
        }
    }

    pub fn mark_aborting(&mut self, index: usize) -> bool {
        let Some(item) = self.items.get_mut(index) else {
            return false;
        };
        if !item.can_abort {
            return false;
        }
        item.can_abort = false;
        item.status = DownloadItemStatus::Aborting;
        true
    }

    pub fn mark_aborted(&mut self, index: usize) {
        if let Some(item) = self.items.get_mut(index) {
            item.percent = 100.0;
            item.can_abort = false;
            item.status = DownloadItemStatus::Aborted;
        }
    }

    pub fn mark_failed(&mut self, index: usize, message: String, retrying: bool) {
        if let Some(item) = self.items.get_mut(index) {
            if !retrying {
                item.percent = 100.0;
                item.can_abort = false;
            }
            item.status = DownloadItemStatus::Failed { message, retrying };
        }
    }
}

pub fn progress_weights(items: &[MediaItem]) -> Vec<f64> {
    let known_sizes = items
        .iter()
        .filter_map(|item| item.estimated_bytes)
        .collect::<Vec<_>>();
    let fallback_weight = if known_sizes.is_empty() {
        1.0
    } else {
        known_sizes
            .iter()
            .fold(0u64, |total, size| total.saturating_add(*size)) as f64
            / known_sizes.len() as f64
    };
    items
        .iter()
        .map(|item| {
            item.estimated_bytes
                .map(|size| size as f64)
                .unwrap_or(fallback_weight)
        })
        .map(|weight| weight.max(1.0))
        .collect()
}

fn initial_progress(items: &[MediaItem]) -> Vec<DownloadItemProgress> {
    items
        .iter()
        .zip(progress_weights(items))
        .map(|(item, weight)| DownloadItemProgress {
            title: item.title.clone(),
            percent: 0.0,
            weight,
            status: DownloadItemStatus::Ready,
            can_abort: true,
        })
        .collect()
}
