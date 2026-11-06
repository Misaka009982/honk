//! 与重载所有者共用的定时器；取消等待不重置期限，完成一次刷新后再开始计时。

use std::time::Duration;

use tokio::time::Instant;

#[derive(Default)]
pub(crate) struct Schedule {
    interval: u64,
    next: Option<Instant>,
}

impl Schedule {
    pub(crate) fn configure(&mut self, interval: u64) {
        if self.interval != interval {
            self.interval = interval;
            self.finish();
        }
    }

    pub(crate) fn finish(&mut self) {
        self.next = (self.interval != 0)
            .then(|| Instant::now().checked_add(Duration::from_secs(self.interval)))
            .flatten();
    }

    pub(crate) async fn wait(&self) {
        match self.next {
            Some(next) => tokio::time::sleep_until(next).await,
            None => std::future::pending().await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn first_refresh_waits_and_missed_periods_do_not_accumulate() {
        let mut schedule = Schedule::default();
        schedule.configure(60);
        assert!(
            tokio::time::timeout(Duration::from_secs(59), schedule.wait())
                .await
                .is_err()
        );
        schedule.configure(60);
        schedule.wait().await;
        tokio::time::advance(Duration::from_secs(300)).await;
        schedule.finish();
        assert!(
            tokio::time::timeout(Duration::from_secs(59), schedule.wait())
                .await
                .is_err()
        );
        schedule.wait().await;
    }

    #[tokio::test(start_paused = true)]
    async fn changing_interval_and_disabling_apply_without_a_restart() {
        let mut schedule = Schedule::default();
        schedule.configure(86400);
        schedule.configure(120);
        assert!(
            tokio::time::timeout(Duration::from_secs(119), schedule.wait())
                .await
                .is_err()
        );
        schedule.wait().await;
        schedule.configure(0);
        assert!(
            tokio::time::timeout(Duration::from_secs(86400), schedule.wait())
                .await
                .is_err()
        );
        schedule.configure(30);
        schedule.wait().await;
    }
}
