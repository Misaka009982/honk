//! 定时规则刷新在配置协调器内串行执行，复用其来源发布与激活流程。

use super::*;
use crate::configuration::ActivationFailure;

impl Worker {
    pub(super) async fn remote_rules_interval(&self) -> u64 {
        if !self.source_managed || self.store.dependency_root().is_none() {
            return 0;
        }
        let has_remote = self
            .service
            .sources
            .accepted
            .read()
            .as_ref()
            .is_some_and(|accepted| crate::remote_rules::has_remote(&accepted.update.sources));
        if has_remote {
            self.active.read().await.assets.rules.interval_secs()
        } else {
            0
        }
    }

    pub(super) async fn refresh_remote_rules(&mut self) {
        if self.service.check_phase(&Work::Sighup).is_err() {
            return;
        }
        let Some(accepted) = self.service.sources.accepted.read().clone() else {
            return;
        };
        let mut stopping = self.stopping.clone();
        let update = tokio::select! {
            biased;
            _ = stopping.wait_for(|stopped| *stopped) => return,
            update = crate::remote_rules::refresh(self.store.entry(), Some(&accepted.update.sources)) => update,
        };
        let update = match update {
            Ok(Some(update)) => update,
            Ok(None) => return,
            Err(error) => {
                tracing::warn!(%error, "远程规则定时刷新失败，保留当前配置");
                return;
            }
        };
        let (candidate, sources, diagnostics) = match self.load_prepared(Some(update)).await {
            Ok(prepared) => prepared,
            Err(_) => {
                tracing::warn!("远程规则候选未通过配置准入，保留当前配置");
                return;
            }
        };
        if *self.stopping.borrow() {
            return;
        }
        honk_config::diagnostic::report_detailed_diagnostics(&diagnostics);
        match self
            .activation
            .activate(ActivationRequest {
                candidate,
                sources,
                diagnostics,
                expected_group_revision: None,
                deferred_provider: None,
            })
            .await
        {
            Ok(outcome) => {
                tracing::info!(generation = outcome.generation(), "远程规则定时刷新已生效");
            }
            Err(ActivationFailure::Degraded(generation)) => {
                tracing::warn!(generation, "远程规则已更新，数据面处于降级状态");
            }
            Err(failure) => tracing::warn!(?failure, "远程规则定时激活未成功"),
        }
    }
}
