use tokio_cron_scheduler::JobScheduler;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct CronScheduler {
    scheduler: Arc<Mutex<JobScheduler>>,
}

impl CronScheduler {
    /// Create a new CronScheduler instance
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let scheduler = JobScheduler::new().await?;
        Ok(Self {
            scheduler: Arc::new(Mutex::new(scheduler)),
        })
    }

    /// Start the scheduler to execute pending jobs
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut scheduler = self.scheduler.lock().await;
        scheduler.start().await?;
        tracing::info!("Cron scheduler started");
        Ok(())
    }

    /// Shutdown the scheduler gracefully
    pub async fn shutdown(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut scheduler = self.scheduler.lock().await;
        scheduler.shutdown().await?;
        tracing::info!("Cron scheduler shutdown");
        Ok(())
    }

    /// Add a fetch job with a cron expression
    pub async fn add_fetch_job(
        &self,
        subscription_id: String,
        fetcher_type: String,
        cron_expression: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use tokio_cron_scheduler::Job;

        let job = Job::new_async(cron_expression, |_uuid, _l| {
            Box::pin(async move {
                tracing::debug!("Cron job executed");
            })
        })?;

        let mut scheduler = self.scheduler.lock().await;
        scheduler.add(job).await?;

        tracing::info!(
            "Added fetch job for subscription {} (fetcher: {}, cron: {})",
            subscription_id,
            fetcher_type,
            cron_expression
        );

        Ok(())
    }
}

impl Clone for CronScheduler {
    fn clone(&self) -> Self {
        Self {
            scheduler: Arc::clone(&self.scheduler),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_scheduler_creation() {
        let scheduler = CronScheduler::new().await;
        assert!(scheduler.is_ok());
    }

    #[tokio::test]
    async fn test_scheduler_start_shutdown() {
        let scheduler = CronScheduler::new().await.unwrap();
        assert!(scheduler.start().await.is_ok());
        assert!(scheduler.shutdown().await.is_ok());
    }

    #[tokio::test]
    async fn test_scheduler_clone() {
        let scheduler1 = CronScheduler::new().await.unwrap();
        let scheduler2 = scheduler1.clone();

        assert!(scheduler2.start().await.is_ok());
        assert!(scheduler2.shutdown().await.is_ok());
    }
}
